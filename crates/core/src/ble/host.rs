//! The BLE host task: TrouBLE on esp-radio's controller, serving HID over
//! GATT (keyboard + consumer control), Battery and Device Information.

use alloc::boxed::Box;
use core::ffi::c_void;
use core::sync::atomic::{AtomicU8, Ordering};

use embassy_futures::select::{select, select3};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal};
use embassy_time::Timer;
use esp_hal::peripherals::BT;
use heapless::String;
use esp_radio::ble::controller::BleConnector;
use esp_rtos::embassy::Executor;
use static_cell::StaticCell;
use trouble_host::prelude::*;

use super::gatt::Server;
use super::keys::{consumer_report, keyboard_report, CONSUMER_RELEASE, KEYBOARD_RELEASE};
use super::{BondRecord, Key, StartConfig, Status, MAX_BONDS, NAME_MAX};

const CONNECTIONS: usize = 1;
/// L2CAP channels: signalling, ATT and the security manager, plus one spare.
const L2CAP_CHANNELS: usize = 4;
/// HCI command slots for the external controller.
const HCI_SLOTS: usize = 20;
/// How long a key is held down before release is sent.
const KEY_HOLD_MS: u64 = 30;
/// Host thread priority: above the game loop (0), so pairing and key
/// sends aren't held up by a busy frame, and below the controller task
/// (max), which must finish handling each command first.
const HOST_PRIORITY: u32 = 1;
/// The task's future lives in static storage; the stack only has to hold
/// polling frames, the deepest being pairing crypto.
const HOST_STACK: usize = 16 * 1024;

// --- Shared with the game loop -----------------------------------------

static START: Signal<CriticalSectionRawMutex, StartConfig> = Signal::new();
static STOP: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static KEYS: Channel<CriticalSectionRawMutex, Key, 4> = Channel::new();
static NEW_BOND: Signal<CriticalSectionRawMutex, BondRecord> = Signal::new();
static STATUS: AtomicU8 = AtomicU8::new(0);

fn set_status(s: Status) {
    let v = match s {
        Status::Off | Status::Unavailable => 0,
        Status::Starting => 1,
        Status::Advertising => 2,
        Status::Connected { secure: false } => 3,
        Status::Connected { secure: true } => 4,
        Status::Failed => 5,
    };
    STATUS.store(v, Ordering::Release);
}

pub fn status() -> Status {
    match STATUS.load(Ordering::Acquire) {
        1 => Status::Starting,
        2 => Status::Advertising,
        3 => Status::Connected { secure: false },
        4 => Status::Connected { secure: true },
        5 => Status::Failed,
        _ => Status::Off,
    }
}

/// Bring the radio up and advertise. Ignored while a session is running.
pub fn start(config: StartConfig) {
    if status() != Status::Off {
        return;
    }
    STOP.reset();
    set_status(Status::Starting);
    START.signal(config);
}

/// End the session and power the radio down. See `stop_and_wait`.
pub fn stop() {
    if status() != Status::Off {
        STOP.signal(());
    }
}

/// Queue a key press. False if not connected or the queue is full.
pub fn send(key: Key) -> bool {
    matches!(status(), Status::Connected { secure: true }) && KEYS.try_send(key).is_ok()
}

/// A bond made since the last call, for the game thread to store.
pub fn take_new_bond() -> Option<BondRecord> {
    NEW_BOND.try_take()
}

/// Start the BLE host thread. Call once at boot, after `esp_rtos::start`.
///
/// The host runs on its own scheduler thread rather than an interrupt
/// executor: the controller's HCI layer must not be re-entered from an
/// interrupt that preempts the controller task. Inside an interrupt, the
/// host answered a Command Complete before the controller had freed its
/// single command buffer, and the controller asserted.
pub fn spawn(bt: BT<'static>) {
    let param = Box::into_raw(Box::new(bt)).cast::<c_void>();
    // SAFETY: `param` is a leaked `Box<BT>` the thread takes ownership of.
    unsafe {
        esp_radio_rtos_driver::task_create("ble-host", ble_thread, param, HOST_PRIORITY, None, HOST_STACK);
    }
}

extern "C" fn ble_thread(param: *mut c_void) {
    // SAFETY: made from `Box<BT>` in `spawn`, and only taken once.
    let bt = *unsafe { Box::from_raw(param.cast::<BT<'static>>()) };
    static EXECUTOR: StaticCell<Executor> = StaticCell::new();
    EXECUTOR.init(Executor::new()).run(move |spawner| match ble_task(bt) {
        Ok(token) => spawner.spawn(token),
        Err(_) => println!("[BLE] could not spawn task"),
    })
}

// --- The task ------------------------------------------------------------

#[embassy_executor::task]
async fn ble_task(mut bt: BT<'static>) {
    // The GATT server is built once and kept for every session: TrouBLE's
    // generated constructor puts the device name and each characteristic's
    // storage in one-shot statics, so a second build panics.
    let mut server: Option<&'static Server<'static>> = None;
    let mut server_failed = false;
    loop {
        let config = START.wait().await;
        if server.is_none() && !server_failed {
            server = build_server(&config);
            server_failed = server.is_none();
        }
        let Some(server) = server else {
            set_status(Status::Failed);
            STOP.wait().await;
            set_status(Status::Off);
            continue;
        };
        match BleConnector::new(bt.reborrow(), Default::default()) {
            Ok(connector) => {
                let controller: ExternalController<_, HCI_SLOTS> = ExternalController::new(connector);
                session(controller, server, &config).await;
            }
            Err(e) => {
                println!("[BLE] controller init failed: {:?}", e);
                set_status(Status::Failed);
                STOP.wait().await;
            }
        }
        // Controller and connector are dropped here, which powers the
        // radio down.
        KEYS.clear();
        set_status(Status::Off);
        println!("[BLE] stopped");
    }
}

/// Build the one GATT server for this boot. Its GAP device name is fixed
/// from the first session (the advertised name is refreshed every session).
fn build_server(config: &StartConfig) -> Option<&'static Server<'static>> {
    static NAME: StaticCell<String<NAME_MAX>> = StaticCell::new();
    static SERVER: StaticCell<Server<'static>> = StaticCell::new();
    let name: &'static str = NAME.init(config.name.clone()).as_str();
    match Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name,
        appearance: &appearance::human_interface_device::KEYBOARD,
    })) {
        Ok(server) => Some(SERVER.init(server)),
        Err(e) => {
            println!("[BLE] GATT server setup failed: {}", e);
            None
        }
    }
}

async fn session<C: Controller>(controller: C, server: &'static Server<'static>, config: &StartConfig) {
    let mut resources: HostResources<C, DefaultPacketPool, CONNECTIONS, L2CAP_CHANNELS, 1, MAX_BONDS> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(Address::random(config.address))
        .set_io_capabilities(IoCapabilities::NoInputNoOutput)
        .build();
    for bond in config.bonds.iter() {
        if stack.add_bond_information(to_bond(bond)).is_err() {
            println!("[BLE] could not restore a bond");
        }
    }
    let mut runner = stack.runner();
    let mut peripheral = stack.peripheral();

    let run = async {
        loop {
            if let Err(e) = runner.run().await {
                println!("[BLE] host error: {:?}", e);
                Timer::after_millis(100).await;
            }
        }
    };
    let app = async {
        loop {
            set_status(Status::Advertising);
            KEYS.clear();
            let conn = match advertise(config.name.as_str(), &mut peripheral, server).await {
                Ok(conn) => conn,
                Err(e) => {
                    println!("[BLE] advertising failed: {:?}", e);
                    Timer::after_secs(1).await;
                    continue;
                }
            };
            let _ = conn.raw().set_bondable(true);
            set_status(Status::Connected { secure: false });
            println!("[BLE] connected");
            select(gatt_events(&conn), send_keys(server, &conn)).await;
            println!("[BLE] disconnected");
        }
    };
    select3(run, app, STOP.wait()).await;
}

async fn advertise<'stack, 'server, C: Controller>(
    name: &str,
    peripheral: &mut Peripheral<'stack, C, DefaultPacketPool>,
    server: &'server Server<'static>,
) -> Result<GattConnection<'stack, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut adv = [0u8; 31];
    let adv_len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteServiceUuids16(&[
                service::HUMAN_INTERFACE_DEVICE.to_le_bytes(),
                service::BATTERY.to_le_bytes(),
            ]),
        ],
        &mut adv[..],
    )?;
    // The name goes in the scan response, where it has more room.
    let mut scan = [0u8; 31];
    let name = &name.as_bytes()[..name.len().min(29)];
    let scan_len = AdStructure::encode_slice(&[AdStructure::CompleteLocalName(name)], &mut scan[..])?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &adv[..adv_len],
                scan_data: &scan[..scan_len],
            },
        )
        .await?;
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    Ok(conn)
}

/// Answer GATT requests and track pairing until the link drops.
async fn gatt_events<P: PacketPool>(conn: &GattConnection<'_, '_, P>) {
    loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => {
                println!("[BLE] link dropped: {:?}", reason);
                return;
            }
            GattConnectionEvent::PairingComplete { bond, .. } => {
                if let Some(bond) = bond {
                    NEW_BOND.signal(from_bond(&bond));
                }
                set_status(Status::Connected { secure: true });
                println!("[BLE] paired");
            }
            GattConnectionEvent::Encrypted { .. } => {
                set_status(Status::Connected { secure: true });
            }
            GattConnectionEvent::PairingFailed(e) => {
                println!("[BLE] pairing failed: {:?}", e);
            }
            GattConnectionEvent::Gatt { event } => match event.accept() {
                Ok(reply) => reply.send().await,
                Err(e) => println!("[BLE] GATT reply failed: {:?}", e),
            },
            _ => {}
        }
    }
}

/// Send queued keys as a press followed shortly by a release.
async fn send_keys<P: PacketPool>(server: &Server<'_>, conn: &GattConnection<'_, '_, P>) {
    loop {
        let key = KEYS.receive().await;
        let result = match key {
            Key::Keyboard(usage) => {
                let report = server.hid.keyboard;
                let pressed = report.notify(conn, &keyboard_report(usage), false).await;
                Timer::after_millis(KEY_HOLD_MS).await;
                pressed.and(report.notify(conn, &KEYBOARD_RELEASE, false).await)
            }
            Key::Consumer(usage) => {
                let report = server.hid.consumer;
                let pressed = report.notify(conn, &consumer_report(usage), false).await;
                Timer::after_millis(KEY_HOLD_MS).await;
                pressed.and(report.notify(conn, &CONSUMER_RELEASE, false).await)
            }
        };
        if result.is_err() {
            return;
        }
    }
}

// --- Bond conversion -------------------------------------------------------

fn to_bond(b: &BondRecord) -> BondInformation {
    let identity = Identity {
        addr: Address { kind: AddrKind::new(b.addr_kind), addr: BdAddr::new(b.addr) },
        irk: b.irk.and_then(core::num::NonZeroU128::new).map(IdentityResolvingKey),
    };
    let level = if b.authenticated { SecurityLevel::EncryptedAuthenticated } else { SecurityLevel::Encrypted };
    BondInformation::new(identity, LongTermKey(b.ltk), level, true)
}

fn from_bond(b: &BondInformation) -> BondRecord {
    let mut addr = [0u8; 6];
    addr.copy_from_slice(b.identity.addr.addr.raw());
    BondRecord {
        addr_kind: b.identity.addr.kind.as_raw(),
        addr,
        irk: b.identity.irk.map(|k| k.0.get()),
        ltk: b.ltk.0,
        authenticated: b.security_level == SecurityLevel::EncryptedAuthenticated,
    }
}
