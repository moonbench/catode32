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
//! `WifiController::new`), runs the scan, and calls `release` to
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

/// Active-scan dwell per channel, in ms. The driver leaves a channel
/// after the minimum if no probe response has arrived, and probe
/// responses get missed often enough that short minimums (esp-radio's
/// default is 10 ms) give empty scans with no error. A minimum above the
/// usual ~102 ms beacon interval means every channel hears at least one
/// beacon either way. A full 2.4 GHz sweep takes about 1.6-2 s.
#[cfg(not(feature = "desktop"))]
const SCAN_DWELL_MIN_MS: u64 = 120;
#[cfg(not(feature = "desktop"))]
const SCAN_DWELL_MAX_MS: u64 = 150;

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
    pub channel: u8,
    /// False for open networks.
    pub secured: bool,
}

/// Drive a single wifi scan. On firmware this brings the radio up,
/// runs the scan, releases, and updates `wifi_familiar` /
/// `wifi_recent` / `in_familiar_location`. On desktop there is no
/// radio; the call returns `None` and leaves the state untouched.
#[cfg(not(feature = "desktop"))]
pub fn scan_now(ctx: &mut GameContext) -> Option<Vec<ScanAp, 32>> {
    let aps = scan_aps(ctx).ok()?;
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

/// Why [`scan_aps`] came back without a list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScanError {
    /// The radio couldn't be brought up (always the case on desktop).
    NoRadio,
    /// The radio started but the scan itself returned an error.
    Failed,
}

/// Run a scan and return what was seen, without touching the familiar /
/// recent tracking. For tools that let the player scan on demand, where
/// repeated scans would otherwise skew the location counts. Blocks for
/// ~1-3 s on firmware.
#[cfg(not(feature = "desktop"))]
pub fn scan_aps(ctx: &mut GameContext) -> Result<Vec<ScanAp, 32>, ScanError> {
    use embassy_futures::block_on;

    if !radio::acquire(ctx) {
        println!("[WiFi] scan_aps: radio unavailable");
        return Err(ScanError::NoRadio);
    }
    let started = crate::platform::time::Instant::now();
    let result = {
        let controller = ctx.wifi.as_mut().expect("acquire succeeded");
        block_on(perform_scan(controller, None))
    };
    radio::release(ctx);
    match result {
        Ok(v) => {
            println!(
                "[WiFi] scan_aps: {} APs in {} ms",
                v.len(),
                started.elapsed().as_millis()
            );
            Ok(v)
        }
        Err(e) => {
            println!("[WiFi] scan_aps failed: {:?}", e);
            Err(ScanError::Failed)
        }
    }
}

#[cfg(feature = "desktop")]
pub fn scan_aps(_ctx: &mut GameContext) -> Result<Vec<ScanAp, 32>, ScanError> {
    Err(ScanError::NoRadio)
}

/// 2.4 GHz channels covered by a [`ChannelSweep`].
pub const SWEEP_CHANNELS: u8 = 13;

/// A scan done one channel per call, so the caller's frame loop keeps
/// running (and animating) between channels instead of blocking for the
/// whole sweep. Each step blocks for about one channel's dwell. Like
/// [`scan_aps`] it leaves the familiar-network tracking alone.
///
/// The radio is held from [`begin`](Self::begin) until the last step or
/// [`abort`](Self::abort); call `abort` when leaving mid-sweep.
pub struct ChannelSweep {
    /// Next channel to scan, 1-based. 0 when no sweep is running.
    next: u8,
    aps: Vec<ScanAp, 32>,
    /// APs seen per channel, index 0 = channel 1.
    per_channel: [u8; SWEEP_CHANNELS as usize],
    started_ms: u64,
}

impl Default for ChannelSweep {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelSweep {
    pub const fn new() -> Self {
        Self { next: 0, aps: Vec::new(), per_channel: [0; SWEEP_CHANNELS as usize], started_ms: 0 }
    }

    pub fn is_running(&self) -> bool {
        self.next != 0
    }

    /// Channels finished so far in the current or last sweep.
    pub fn channels_done(&self) -> u8 {
        if self.next == 0 { SWEEP_CHANNELS } else { self.next - 1 }
    }

    /// APs found, strongest reading per BSSID.
    pub fn results(&self) -> &[ScanAp] {
        &self.aps
    }

    pub fn per_channel(&self) -> &[u8; SWEEP_CHANNELS as usize] {
        &self.per_channel
    }

    /// Bring the radio up and start from channel 1.
    pub fn begin(&mut self, ctx: &mut GameContext) -> Result<(), ScanError> {
        self.abort(ctx);
        self.aps.clear();
        self.per_channel = [0; SWEEP_CHANNELS as usize];
        if !radio_acquire(ctx) {
            println!("[WiFi] sweep: radio unavailable");
            return Err(ScanError::NoRadio);
        }
        self.next = 1;
        self.started_ms = crate::platform::time::now_ms();
        Ok(())
    }

    /// Scan the next channel. `Ok(true)` once the sweep is complete, at
    /// which point the radio has been released.
    pub fn step(&mut self, ctx: &mut GameContext) -> Result<bool, ScanError> {
        if self.next == 0 {
            return Ok(true);
        }
        let channel = self.next;
        let found = match scan_channel(ctx, channel) {
            Ok(found) => found,
            Err(e) => {
                self.abort(ctx);
                return Err(e);
            }
        };
        self.per_channel[channel as usize - 1] = found.len().min(u8::MAX as usize) as u8;
        for ap in found {
            match self.aps.iter_mut().find(|a| a.bssid == ap.bssid) {
                // Overlapping channels can hear the same AP twice.
                Some(seen) => {
                    if ap.rssi > seen.rssi {
                        *seen = ap;
                    }
                }
                None => {
                    let _ = self.aps.push(ap);
                }
            }
        }
        if channel >= SWEEP_CHANNELS {
            self.next = 0;
            radio_release(ctx);
            println!(
                "[WiFi] sweep: {} APs in {} ms",
                self.aps.len(),
                crate::platform::time::now_ms() - self.started_ms
            );
            Ok(true)
        } else {
            self.next += 1;
            Ok(false)
        }
    }

    /// Stop a sweep part way and release the radio. Safe to call when
    /// nothing is running.
    pub fn abort(&mut self, ctx: &mut GameContext) {
        if self.next != 0 {
            self.next = 0;
            radio_release(ctx);
        }
    }
}

/// Repeated scans of a single channel, for following one network's
/// signal. Holds the radio from [`begin`](Self::begin) to
/// [`end`](Self::end), so each [`scan`](Self::scan) costs only one
/// channel's dwell (~150 ms) rather than a radio start-up. Like
/// [`scan_aps`] it leaves the familiar-network tracking alone.
pub struct ChannelWatch {
    /// The channel being watched; 0 when not running.
    channel: u8,
}

impl Default for ChannelWatch {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelWatch {
    pub const fn new() -> Self {
        Self { channel: 0 }
    }

    pub fn is_running(&self) -> bool {
        self.channel != 0
    }

    /// Bring the radio up to watch `channel`.
    pub fn begin(&mut self, ctx: &mut GameContext, channel: u8) -> Result<(), ScanError> {
        self.end(ctx);
        if channel == 0 {
            return Err(ScanError::Failed);
        }
        if !radio_acquire(ctx) {
            println!("[WiFi] watch: radio unavailable");
            return Err(ScanError::NoRadio);
        }
        self.channel = channel;
        Ok(())
    }

    /// Scan the watched channel once. Blocks for one channel's dwell.
    pub fn scan(&mut self, ctx: &mut GameContext) -> Result<Vec<ScanAp, 32>, ScanError> {
        if self.channel == 0 {
            return Err(ScanError::NoRadio);
        }
        scan_channel(ctx, self.channel)
    }

    /// Release the radio. Safe to call when nothing is running.
    pub fn end(&mut self, ctx: &mut GameContext) {
        if self.channel != 0 {
            self.channel = 0;
            radio_release(ctx);
        }
    }
}

#[cfg(not(feature = "desktop"))]
fn radio_acquire(ctx: &mut GameContext) -> bool {
    radio::acquire(ctx)
}

#[cfg(not(feature = "desktop"))]
fn radio_release(ctx: &mut GameContext) {
    radio::release(ctx);
}

#[cfg(not(feature = "desktop"))]
fn scan_channel(ctx: &mut GameContext, channel: u8) -> Result<Vec<ScanAp, 32>, ScanError> {
    use embassy_futures::block_on;
    let Some(controller) = ctx.wifi.as_mut() else {
        return Err(ScanError::NoRadio);
    };
    block_on(perform_scan(controller, Some(channel))).map_err(|e| {
        println!("[WiFi] sweep: channel {} failed: {:?}", channel, e);
        ScanError::Failed
    })
}

#[cfg(feature = "desktop")]
fn radio_acquire(_ctx: &mut GameContext) -> bool {
    false
}

#[cfg(feature = "desktop")]
fn radio_release(_ctx: &mut GameContext) {}

#[cfg(feature = "desktop")]
fn scan_channel(_ctx: &mut GameContext, _channel: u8) -> Result<Vec<ScanAp, 32>, ScanError> {
    Err(ScanError::NoRadio)
}

#[cfg(not(feature = "desktop"))]
async fn perform_scan(
    controller: &mut esp_radio::wifi::WifiController<'static>,
    channel: Option<u8>,
) -> Result<Vec<ScanAp, 32>, esp_radio::wifi::WifiError> {
    use esp_radio::wifi::{
        scan::{ScanConfig, ScanTypeConfig},
        sta::StationConfig,
        Config as WifiConfig,
    };

    // Apply a station config. This selects STA mode for a freshly
    // initialized controller. Cheap on subsequent calls within the
    // same `radio::acquire` lifetime.
    let sta_config = WifiConfig::Station(StationConfig::default());
    controller.set_config(&sta_config)?;

    let mut scan_config = ScanConfig::default()
        .with_max(SCAN_MAX_APS)
        .with_scan_type(ScanTypeConfig::Active {
            min: Duration::from_millis(SCAN_DWELL_MIN_MS),
            max: Duration::from_millis(SCAN_DWELL_MAX_MS),
        });
    if let Some(channel) = channel {
        scan_config = scan_config.with_channel(channel);
    }
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
            channel: ap.channel,
            secured: !matches!(
                ap.auth_method,
                None | Some(esp_radio::wifi::AuthenticationMethod::None)
            ),
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
