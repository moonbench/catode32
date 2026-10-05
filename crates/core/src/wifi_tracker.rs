//! WiFi-based location tracking.
//!
//! Maintains two lists of access points on the game context:
//!
//! * `wifi_familiar`: up to [`WIFI_FAMILIAR_MAX`] APs seen most often.
//! * `wifi_recent`: up to [`WIFI_RECENT_MAX`] APs seen recently but not yet
//!   promoted.
//!
//! Each scan increments the count for any visible AP and decays the count
//! for every other tracked entry. Entries decayed below 0 are pruned; recent
//! entries past a threshold are promoted into the familiar set.
//!
//! `in_familiar_location` is set true on the context whenever a familiar
//! AP is visible in the latest scan. That's the "is the cat at home?"
//! signal behaviors gate on.
//!
//! Scans are driven from the game-loop's transition midpoint (see
//! `Game::maybe_scan_wifi`) so the ~1-3 s blocking call is hidden behind a
//! black screen.
//!
//! Power: the radio is fully off at rest. `scan_now` calls
//! [`crate::radio::acquire`] to bring it up (which does a full
//! `esp_radio::wifi::new`), runs the scan, and calls `release` to
//! drop the controller and park the peripheral again. If ESP-NOW has
//! an active session the refcount keeps the radio up through the scan
//! and the release only decrements.

use crate::platform::time::Duration;
use heapless::Vec;

use crate::context::{GameContext, WIFI_SSID_MAX};
#[cfg(not(feature = "desktop"))]
use crate::context::{WifiEntry, WIFI_FAMILIAR_MAX, WIFI_RECENT_MAX};

#[cfg(not(feature = "desktop"))]
use crate::radio;

/// Interval between hourly wifi scans, in real time.
pub const SCAN_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// How many APs to ask the driver for per scan.
#[cfg(not(feature = "desktop"))]
const SCAN_MAX_APS: usize = 20;

/// Count subtracted from every unseen entry each scan.
#[cfg(not(feature = "desktop"))]
const DECAY_PER_SCAN: f32 = 0.25;
/// Min count before a recent entry can be promoted.
#[cfg(not(feature = "desktop"))]
const PROMOTE_MIN: f32 = 5.0;

/// Snapshot of the latest scan, surfaced to the debug scene so it can show
/// signal strength + auth mode alongside the familiar/recent lists.
#[derive(Clone)]
pub struct ScanAp {
    pub bssid: [u8; 6],
    pub ssid: heapless::String<WIFI_SSID_MAX>,
    pub rssi: i8,
}

/// Drive a single wifi scan. On firmware this brings the radio up,
/// runs the scan, releases, and updates `wifi_familiar` /
/// `wifi_recent` / `in_familiar_location`. On desktop there is no
/// radio; the call returns `None` and leaves the state untouched.
#[cfg(not(feature = "desktop"))]
pub fn scan_now(ctx: &mut GameContext) -> Option<Vec<ScanAp, 32>> {
    use embassy_futures::block_on;

    if !radio::acquire(ctx) {
        return None;
    }
    let aps = {
        let controller = ctx.wifi.as_mut().expect("acquire succeeded");
        match block_on(perform_scan(controller)) {
            Ok(v) => v,
            Err(e) => {
                println!("[WiFi] Scan failed: {:?}", e);
                radio::release(ctx);
                return None;
            }
        }
    };
    radio::release(ctx);

    process(ctx, &aps);

    println!(
        "[WiFi] Scan done. familiar={} ({}/{} known)",
        ctx.in_familiar_location,
        ctx.wifi_familiar.len(),
        WIFI_FAMILIAR_MAX,
    );
    Some(aps)
}

#[cfg(feature = "desktop")]
pub fn scan_now(_ctx: &mut GameContext) -> Option<Vec<ScanAp, 32>> {
    None
}

#[cfg(not(feature = "desktop"))]
async fn perform_scan(
    controller: &mut esp_radio::wifi::WifiController<'static>,
) -> Result<Vec<ScanAp, 32>, esp_radio::wifi::WifiError> {
    use esp_radio::wifi::{scan::ScanConfig, sta::StationConfig, Config as WifiConfig};

    // Apply a station config. This selects STA mode for a freshly
    // initialized controller. Cheap on subsequent calls within the
    // same `radio::acquire` lifetime.
    let sta_config = WifiConfig::Station(StationConfig::default());
    controller.set_config(&sta_config)?;

    let scan_config = ScanConfig::default().with_max(SCAN_MAX_APS);
    let result = controller.scan_async(&scan_config).await?;

    let mut out: Vec<ScanAp, 32> = Vec::new();
    for ap in result.iter() {
        let mut ssid: heapless::String<WIFI_SSID_MAX> = heapless::String::new();
        for c in ap.ssid.as_str().chars() {
            if ssid.push(c).is_err() {
                break;
            }
        }
        let entry = ScanAp {
            bssid: ap.bssid,
            ssid,
            rssi: ap.signal_strength,
        };
        if out.push(entry).is_err() {
            break;
        }
    }
    Ok(out)
}

/// Decay-and-promote algorithm. Public so the debug scene's "Scan" button
/// (or a unit test, eventually) can drive it with a synthesized AP list.
#[cfg(not(feature = "desktop"))]
fn process(ctx: &mut GameContext, aps: &[ScanAp]) {
    // Decay unseen entries, then prune the ones that hit zero.
    for entry in ctx.wifi_familiar.iter_mut() {
        if !aps.iter().any(|ap| ap.bssid == entry.bssid) {
            entry.count -= DECAY_PER_SCAN;
        }
    }
    for entry in ctx.wifi_recent.iter_mut() {
        if !aps.iter().any(|ap| ap.bssid == entry.bssid) {
            entry.count -= DECAY_PER_SCAN;
        }
    }
    ctx.wifi_familiar.retain(|e| e.count > 0.0);
    ctx.wifi_recent.retain(|e| e.count > 0.0);

    // Increment seen entries; route genuinely new ones into the recent list,
    // evicting the lowest-count entry only when its score is below the
    // newcomer's.
    for ap in aps.iter() {
        if let Some(e) = ctx
            .wifi_familiar
            .iter_mut()
            .find(|e| e.bssid == ap.bssid)
        {
            e.count += 1.0;
            continue;
        }
        if let Some(e) = ctx.wifi_recent.iter_mut().find(|e| e.bssid == ap.bssid) {
            e.count += 1.0;
            continue;
        }
        let new_entry = WifiEntry {
            bssid: ap.bssid,
            ssid: ap.ssid.clone(),
            count: 1.0,
        };
        if ctx.wifi_recent.len() < WIFI_RECENT_MAX {
            let _ = ctx.wifi_recent.push(new_entry);
        } else if let Some(victim_idx) = lowest_idx(&ctx.wifi_recent) {
            if ctx.wifi_recent[victim_idx].count < new_entry.count {
                ctx.wifi_recent[victim_idx] = new_entry;
            }
        }
    }

    // Promote: recent entries that have hit the threshold. Swap the weakest
    // familiar entry out when full, demoting it into recent only if recent
    // has room (otherwise it falls out of tracking entirely).
    let mut i = 0;
    while i < ctx.wifi_recent.len() {
        if ctx.wifi_recent[i].count < PROMOTE_MIN {
            i += 1;
            continue;
        }
        let candidate = ctx.wifi_recent.remove(i);
        if ctx.wifi_familiar.len() < WIFI_FAMILIAR_MAX {
            let _ = ctx.wifi_familiar.push(candidate);
        } else if let Some(weakest_idx) = lowest_idx(&ctx.wifi_familiar) {
            if candidate.count > ctx.wifi_familiar[weakest_idx].count {
                let demoted =
                    core::mem::replace(&mut ctx.wifi_familiar[weakest_idx], candidate);
                if ctx.wifi_recent.len() < WIFI_RECENT_MAX {
                    let _ = ctx.wifi_recent.push(demoted);
                }
                // If recent is also full, the demoted entry is dropped.
            } else {
                // Candidate is weaker than the weakest familiar, put it
                // back where it came from.
                let _ = ctx.wifi_recent.insert(i, candidate);
                i += 1;
            }
        }
    }

    // Update "at home?" flag.
    ctx.in_familiar_location = ctx
        .wifi_familiar
        .iter()
        .any(|e| aps.iter().any(|ap| ap.bssid == e.bssid));
}

#[cfg(not(feature = "desktop"))]
fn lowest_idx<const N: usize>(v: &Vec<WifiEntry, N>) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, e) in v.iter().enumerate() {
        match best {
            None => best = Some((i, e.count)),
            Some((_, c)) if e.count < c => best = Some((i, e.count)),
            _ => {}
        }
    }
    best.map(|(i, _)| i)
}

/// Format a BSSID as `aa:bb:cc:dd:ee:ff`. Used by the save layer and the
/// debug scene.
pub fn format_bssid(b: &[u8; 6]) -> heapless::String<17> {
    use core::fmt::Write;
    let mut s: heapless::String<17> = heapless::String::new();
    for (i, byte) in b.iter().enumerate() {
        if i > 0 {
            let _ = s.push(':');
        }
        let _ = write!(s, "{:02x}", byte);
    }
    s
}

/// Parse `aa:bb:cc:dd:ee:ff` (case-insensitive). Returns None if malformed.
/// Used by the save layer to reconstitute persisted wifi entries.
pub fn parse_bssid(s: &str) -> Option<[u8; 6]> {
    let mut out = [0u8; 6];
    let mut parts = s.split(':');
    for slot in out.iter_mut() {
        let part = parts.next()?;
        if part.len() != 2 {
            return None;
        }
        *slot = u8::from_str_radix(part, 16).ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some(out)
}
