//! Verifies the desktop storage backend (the file-backed partition image
//! `./catode32-save.bin`) is created by a save and removed by an erase.
//!
//! Runs the test inside a temp working directory so a real save file in
//! the repo root isn't clobbered. Single-test file so cargo runs it in
//! its own process. The static `FLASH_STORAGE` cell on firmware has no
//! desktop analogue, but cargo's per-binary process isolation keeps
//! multiple test files from racing on the same save files.

#![cfg(feature = "desktop")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use catode32_core::{context::GameContext, led::Led, save, storage};

/// Serialises the tests in this file. They both flip the process CWD and
/// then read/write the save files. Running in parallel would
/// have them racing on the same path.
fn lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

struct CwdGuard {
    prev: PathBuf,
    tmp: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl CwdGuard {
    fn enter() -> Self {
        let _lock = lock();
        let prev = env::current_dir().expect("get cwd");
        let mut tmp = env::temp_dir();
        tmp.push(format!(
            "catode32-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&tmp).expect("create tmp");
        env::set_current_dir(&tmp).expect("set cwd");
        Self { prev, tmp, _lock }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.prev);
        let _ = fs::remove_dir_all(&self.tmp);
    }
}

#[test]
fn save_creates_image_and_erase_removes_it() {
    let _g = CwdGuard::enter();
    storage::init();

    assert!(!storage::has_save(), "tmp dir should start clean");

    let mut ctx = GameContext::new(Led::new());
    assert!(save::save(&mut ctx), "save should succeed");
    assert!(storage::has_save(), "has_save should report the new snapshot");
    assert_eq!(
        fs::metadata(storage::IMAGE_PATH).expect("image").len(),
        6 * 4096,
        "the desktop image stands in for the 24 KB nvs partition"
    );

    assert!(storage::erase_all());
    assert!(!storage::has_save());

    // Erase on an already-clean slot is a success no-op.
    assert!(storage::erase_all());
}
