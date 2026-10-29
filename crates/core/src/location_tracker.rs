//! WiFi-based location tracking.
//!
//! Maintains two lists of access points on the game context:
//!
//! * `loc_familiar`: up to [`LOCATION_FAMILIAR_MAX`] APs seen most often.
//! * `loc_recent`: up to [`LOCATION_RECENT_MAX`] APs seen recently but not yet
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
//! `WifiController::new`), runs the scan, and calls `release` to
//! drop the controller and park the peripheral again. If ESP-NOW has
//! an active session the refcount keeps the radio up through the scan
//! and the release only decrements.

#![cfg_attr(any(feature = "h2", feature = "desktop"), allow(dead_code, unused_imports))]

use crate::platform::time::Duration;
use heapless::Vec;

use crate::context::{GameContext, LOCATION_NAME_MAX};
use crate::context::{LocationEntry, LOCATION_FAMILIAR_MAX, LOCATION_RECENT_MAX};

#[cfg(not(feature = "desktop"))]
use crate::radio;

/// Interval between hourly wifi scans, in real time.
pub const SCAN_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// How many APs to ask the driver for per scan.
#[cfg(not(feature = "desktop"))]
const SCAN_MAX_APS: usize = 20;

/// Count subtracted from every unseen entry each scan.
const DECAY_PER_SCAN: f32 = 0.25;
/// Min count before a recent entry can be promoted.
const PROMOTE_MIN: f32 = 5.0;

/// Familiar entries that must be visible to count as "home". WiFi keeps the
/// original any-one-AP behaviour.
pub const WIFI_QUORUM: usize = 1;

/// A source of location fingerprints (WiFi APs, BLE devices, ...). Entries
/// reuse [`ScanBeacon`]: `addr` holds the 6-byte address and `name` an optional
/// short name.
pub trait LocationSensor {
    /// Minimum visible familiar entries needed to be "at home".
    fn quorum(&self) -> usize;
    /// Run one scan and update the context; `None` if the radio is unavailable.
    fn scan(&mut self, ctx: &mut GameContext) -> Option<Vec<ScanBeacon, 32>>;
}

/// WiFi access-point sensor used on C3/C6.
pub struct WifiSensor;

impl LocationSensor for WifiSensor {
    fn quorum(&self) -> usize {
        WIFI_QUORUM
    }
    fn scan(&mut self, ctx: &mut GameContext) -> Option<Vec<ScanBeacon, 32>> {
        scan_now(ctx)
    }
}

/// Snapshot of the latest scan, surfaced to the debug scene so it can show
/// signal strength + auth mode alongside the familiar/recent lists.
#[derive(Clone)]
pub struct ScanBeacon {
    pub addr: [u8; 6],
    pub name: heapless::String<LOCATION_NAME_MAX>,
    pub rssi: i8,
}

/// Drive a single wifi scan. On firmware this brings the radio up,
/// runs the scan, releases, and updates `loc_familiar` /
/// `loc_recent` / `in_familiar_location`. On desktop there is no
/// radio; the call returns `None` and leaves the state untouched.
#[cfg(not(any(feature = "desktop", feature = "h2")))]
pub fn scan_now(ctx: &mut GameContext) -> Option<Vec<ScanBeacon, 32>> {
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

    process(ctx, &aps, WIFI_QUORUM);

    println!(
        "[WiFi] Scan done. familiar={} ({}/{} known)",
        ctx.in_familiar_location,
        ctx.loc_familiar.len(),
        LOCATION_FAMILIAR_MAX,
    );
    Some(aps)
}

#[cfg(feature = "desktop")]
pub fn scan_now(_ctx: &mut GameContext) -> Option<Vec<ScanBeacon, 32>> {
    None
}

/// Length of one passive BLE scan.
#[cfg(feature = "h2")]
const BLE_SCAN_MS: u64 = 3000;

#[cfg(feature = "h2")]
struct Collector {
    seen: core::cell::RefCell<Vec<ScanBeacon, 32>>,
}

#[cfg(feature = "h2")]
impl trouble_host::prelude::EventHandler for Collector {
    fn on_adv_reports(&self, reports: bt_hci::param::LeAdvReportsIter) {
        let batch = reports.filter_map(|r| r.ok()).filter_map(|r| {
            let addr: [u8; 6] = r.addr.raw().try_into().ok()?;
            Some((r.addr_kind.as_raw(), addr, r.rssi, r.data))
        });
        let kept = crate::ble_filter::filter_reports(batch);
        {
            let mut seen = self.seen.borrow_mut();
            for ap in kept {
                if let Some(e) = seen.iter_mut().find(|e| e.addr == ap.addr) {
                    if ap.rssi > e.rssi {
                        e.rssi = ap.rssi;
                    }
                } else if !seen.is_full() {
                    let _ = seen.push(ap);
                }
            }
        }
    }
}

/// One passive BLE scan. Brings the controller up, listens for
/// `BLE_SCAN_MS`, then drops it (which powers the radio down).
#[cfg(feature = "h2")]
pub fn scan_now(ctx: &mut GameContext) -> Option<Vec<ScanBeacon, 32>> {
    use crate::platform::radio::WIFI;
    use embassy_futures::{block_on, select::select};
    use esp_radio::ble::controller::BleConnector;
    use trouble_host::prelude::*;

    let bt = ctx.radio_peripheral.take()?;
    let connector = match BleConnector::new(bt, Default::default()) {
        Ok(c) => c,
        Err(e) => {
            println!("[BLE] init failed: {:?}", e);
            ctx.radio_peripheral = Some(unsafe { WIFI::steal() });
            return None;
        }
    };

    let collector = Collector {
        seen: core::cell::RefCell::new(Vec::new()),
    };
    {
        let controller: bt_hci::controller::ExternalController<_, 4> =
            bt_hci::controller::ExternalController::new(connector);
        let mut resources: HostResources<_, trouble_host::prelude::DefaultPacketPool, 0, 0> = HostResources::new();
        let stack = trouble_host::new(controller, &mut resources).build();
        let mut runner = stack.runner();
        let mut scanner = Scanner::new(stack.central());
        let config = ScanConfig {
            active: false,
            interval: embassy_time::Duration::from_millis(100),
            window: embassy_time::Duration::from_millis(100),
            ..Default::default()
        };
        let work = async {
            let _session = scanner.scan(&config).await.ok()?;
            // block_on's waker isn't an embassy-executor waker, so
            // embassy_time::Timer would panic. Poll the clock instead.
            let end = embassy_time::Instant::now() + embassy_time::Duration::from_millis(BLE_SCAN_MS);
            core::future::poll_fn(|cx| {
                if embassy_time::Instant::now() >= end {
                    core::task::Poll::Ready(())
                } else {
                    cx.waker().wake_by_ref();
                    core::task::Poll::Pending
                }
            })
            .await;
            Some(())
        };
        let done = block_on(select(runner.run_with_handler(&collector), work));
        if let embassy_futures::select::Either::First(Err(_)) = done {
            println!("[BLE] runner error");
        }
    }
    ctx.radio_peripheral = Some(unsafe { WIFI::steal() });

    let aps = collector.seen.borrow().clone();
    process(ctx, &aps, crate::ble_filter::BLE_QUORUM);
    println!(
        "[BLE] Scan done. {} beacons, familiar={} ({}/{} known)",
        aps.len(),
        ctx.in_familiar_location,
        ctx.loc_familiar.len(),
        LOCATION_FAMILIAR_MAX,
    );
    Some(aps)
}

#[cfg(not(any(feature = "desktop", feature = "h2")))]
async fn perform_scan(
    controller: &mut esp_radio::wifi::WifiController<'static>,
) -> Result<Vec<ScanBeacon, 32>, esp_radio::wifi::WifiError> {
    use esp_radio::wifi::{scan::ScanConfig, sta::StationConfig, Config as WifiConfig};

    // Apply a station config. This selects STA mode for a freshly
    // initialized controller. Cheap on subsequent calls within the
    // same `radio::acquire` lifetime.
    let sta_config = WifiConfig::Station(StationConfig::default());
    controller.set_config(&sta_config)?;

    let scan_config = ScanConfig::default().with_max(SCAN_MAX_APS);
    let result = controller.scan_async(&scan_config).await?;

    let mut out: Vec<ScanBeacon, 32> = Vec::new();
    for ap in result.iter() {
        let mut name: heapless::String<LOCATION_NAME_MAX> = heapless::String::new();
        for c in ap.ssid.as_str().chars() {
            if name.push(c).is_err() {
                break;
            }
        }
        let entry = ScanBeacon {
            addr: ap.bssid,
            name,
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
#[cfg_attr(not(test), allow(dead_code))]
pub fn process(ctx: &mut GameContext, aps: &[ScanBeacon], quorum: usize) {
    // Decay unseen entries, then prune the ones that hit zero.
    for entry in ctx.loc_familiar.iter_mut() {
        if !aps.iter().any(|ap| ap.addr == entry.addr) {
            entry.count -= DECAY_PER_SCAN;
        }
    }
    for entry in ctx.loc_recent.iter_mut() {
        if !aps.iter().any(|ap| ap.addr == entry.addr) {
            entry.count -= DECAY_PER_SCAN;
        }
    }
    ctx.loc_familiar.retain(|e| e.count > 0.0);
    ctx.loc_recent.retain(|e| e.count > 0.0);

    // Increment seen entries; route genuinely new ones into the recent list,
    // evicting the lowest-count entry only when its score is below the
    // newcomer's.
    for ap in aps.iter() {
        if let Some(e) = ctx
            .loc_familiar
            .iter_mut()
            .find(|e| e.addr == ap.addr)
        {
            e.count += 1.0;
            continue;
        }
        if let Some(e) = ctx.loc_recent.iter_mut().find(|e| e.addr == ap.addr) {
            e.count += 1.0;
            continue;
        }
        let new_entry = LocationEntry {
            addr: ap.addr,
            name: ap.name.clone(),
            count: 1.0,
        };
        if ctx.loc_recent.len() < LOCATION_RECENT_MAX {
            let _ = ctx.loc_recent.push(new_entry);
        } else if let Some(victim_idx) = lowest_idx(&ctx.loc_recent) {
            if ctx.loc_recent[victim_idx].count < new_entry.count {
                ctx.loc_recent[victim_idx] = new_entry;
            }
        }
    }

    // Promote: recent entries that have hit the threshold. Swap the weakest
    // familiar entry out when full, demoting it into recent only if recent
    // has room (otherwise it falls out of tracking entirely).
    let mut i = 0;
    while i < ctx.loc_recent.len() {
        if ctx.loc_recent[i].count < PROMOTE_MIN {
            i += 1;
            continue;
        }
        let candidate = ctx.loc_recent.remove(i);
        if ctx.loc_familiar.len() < LOCATION_FAMILIAR_MAX {
            let _ = ctx.loc_familiar.push(candidate);
        } else if let Some(weakest_idx) = lowest_idx(&ctx.loc_familiar) {
            if candidate.count > ctx.loc_familiar[weakest_idx].count {
                let demoted =
                    core::mem::replace(&mut ctx.loc_familiar[weakest_idx], candidate);
                if ctx.loc_recent.len() < LOCATION_RECENT_MAX {
                    let _ = ctx.loc_recent.push(demoted);
                }
                // If recent is also full, the demoted entry is dropped.
            } else {
                // Candidate is weaker than the weakest familiar, put it
                // back where it came from.
                let _ = ctx.loc_recent.insert(i, candidate);
                i += 1;
            }
        }
    }

    // Update "at home?" flag.
    ctx.in_familiar_location = ctx
        .loc_familiar
        .iter()
        .filter(|e| aps.iter().any(|ap| ap.addr == e.addr))
        .count()
        >= quorum.max(1);
}

fn lowest_idx<const N: usize>(v: &Vec<LocationEntry, N>) -> Option<usize> {
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

/// Format a 6-byte address as `aa:bb:cc:dd:ee:ff`. Used by the save layer and the
/// debug scene.
pub fn format_addr(b: &[u8; 6]) -> heapless::String<17> {
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
pub fn parse_addr(s: &str) -> Option<[u8; 6]> {
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
