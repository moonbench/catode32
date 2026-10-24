//! BLE implementation of [`Transport`] for the ESP32-H2.
//!
//! [`Shared`] holds all state and logic that does not touch the radio
//! (advert dedupe, inbox/outbox, who we want to connect to) and is unit
//! tested on desktop. `BleManager` (H2 only) drives a trouble-host stack
//! as a manually polled future: the game loop calls `poll()` and each call
//! gives the stack a few turns, so no executor or extra task is needed.
//!
//! Messaging model (see `ble_proto`):
//! * broadcasts ride in the connectable advert and are picked up by scans;
//! * directed messages use a GATT link where the inviter (first to
//!   `send_to` a peer) is the central and the invitee the peripheral.
//!
//! The radio time-slices between advertising and scanning, so latency is a
//! second or two. That is fine for the pet-visit protocol.

use heapless::Vec;

use crate::ble_proto::{self, ADV_PAYLOAD_MAX, GATT_PAYLOAD_MAX};
use crate::transport::{InboxItem, PeerId, INBOX_CAPACITY};

const SEEN_MAX: usize = 8;
const OUTBOX_MAX: usize = 4;

type Frame = Vec<u8, GATT_PAYLOAD_MAX>;

/// Radio-independent transport state.
pub struct Shared {
    pub own: u16,
    pub active: bool,
    seq: u8,
    adv: Vec<u8, ADV_PAYLOAD_MAX>,
    seen: Vec<(u16, u8), SEEN_MAX>,
    inbox: Vec<InboxItem, INBOX_CAPACITY>,
    outbox: Vec<(u16, Frame), OUTBOX_MAX>,
    pub want: Option<u16>,
    pub link: Option<u16>,
    /// Passkey shown to the player while a pairing confirm is pending.
    pub pair_code: Option<u32>,
    pub pair_answer: Option<bool>,
}

impl Shared {
    pub const fn new(own: u16) -> Self {
        Self {
            own,
            active: false,
            seq: 0,
            adv: Vec::new(),
            seen: Vec::new(),
            inbox: Vec::new(),
            outbox: Vec::new(),
            want: None,
            link: None,
            pair_code: None,
            pair_answer: None,
        }
    }

    /// Replace the broadcast carried in our advert. Returns false if too long.
    pub fn set_broadcast(&mut self, data: &[u8]) -> bool {
        let Ok(v) = Vec::from_slice(data) else {
            return false;
        };
        self.adv = v;
        self.seq = self.seq.wrapping_add(1);
        true
    }

    /// Encode our current advert. Returns the number of bytes used.
    pub fn advert(&self, out: &mut [u8; ble_proto::ADVERT_MAX]) -> usize {
        ble_proto::encode_advert(PeerId::from_short(self.own), self.seq, &self.adv, out).unwrap_or(0)
    }

    /// Queue a directed frame and ask the driver to connect if there is no
    /// link to that peer yet. Returns false if the frame was dropped.
    pub fn queue(&mut self, peer: u16, data: &[u8]) -> bool {
        let Ok(frame) = Frame::from_slice(data) else {
            return false;
        };
        if self.outbox.is_full() {
            self.outbox.remove(0);
        }
        let _ = self.outbox.push((peer, frame));
        if self.link != Some(peer) && self.want.is_none() {
            self.want = Some(peer);
        }
        true
    }

    /// Next queued frame for `peer`, oldest first.
    pub fn take_outbox(&mut self, peer: u16) -> Option<Frame> {
        let i = self.outbox.iter().position(|(p, _)| *p == peer)?;
        Some(self.outbox.remove(i).1)
    }

    pub fn has_outbox(&self, peer: u16) -> bool {
        self.outbox.iter().any(|(p, _)| *p == peer)
    }

    pub fn drop_outbox(&mut self, peer: u16) {
        self.outbox.retain(|(p, _)| *p != peer);
    }

    pub fn clear_outbox(&mut self) {
        self.outbox.clear();
    }

    /// Deliver a frame received over a GATT link.
    pub fn push_inbox(&mut self, src: u16, data: &[u8]) {
        let Ok(data) = Vec::from_slice(data) else {
            return;
        };
        if self.inbox.is_full() {
            self.inbox.remove(0);
        }
        let _ = self.inbox.push(InboxItem {
            src: PeerId::from_short(src),
            data,
        });
    }

    /// Handle a scanned advertisement. A new (id, seq) with a payload is a
    /// broadcast frame; repeats of the same advert are ignored.
    /// Returns `(peer, seq, payload_len)` when the advert was new.
    pub fn ingest_advert(&mut self, adv: &[u8]) -> Option<(u16, u16, usize)> {
        let (id, seq, payload) = ble_proto::parse_advert(adv)?;
        let id = ble_proto::short_id(id);
        if id == self.own {
            return None;
        }
        match self.seen.iter_mut().find(|(i, _)| *i == id) {
            Some(e) if e.1 == seq => return None,
            Some(e) => e.1 = seq,
            None => {
                if self.seen.is_full() {
                    self.seen.remove(0);
                }
                let _ = self.seen.push((id, seq));
            }
        }
        if !payload.is_empty() {
            self.push_inbox(id, payload);
        }
        Some((id, seq as u16, payload.len()))
    }

    pub fn drain(&mut self) -> Vec<InboxItem, INBOX_CAPACITY> {
        core::mem::take(&mut self.inbox)
    }
}

#[cfg(feature = "h2")]
pub use driver::BleManager;

#[cfg(feature = "h2")]
mod driver {
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use core::cell::RefCell;
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll, Waker};

    use embassy_futures::join::join;
    use embassy_futures::select::{select, Either};
    use embassy_time::{Duration, Instant, Timer};
    use esp_radio::ble::controller::BleConnector;
    use heapless::Vec;
    use trouble_host::prelude::*;

    use super::Shared;
    use crate::ble_proto::{self, GATT_PAYLOAD_MAX, RX_CHAR_UUID, SERVICE_UUID, TX_CHAR_UUID};
    use crate::transport::{InboxItem, PeerId, Transport, INBOX_CAPACITY};

    const ADVERTISE_MS: u64 = 1500;
    const SCAN_MS: u64 = 1500;
    const CONNECT_TIMEOUT_MS: u64 = 6000;
    const LINK_IDLE_MS: u64 = 20_000;
    /// Player's window to confirm the pairing code; kept under LINK_IDLE_MS.
    const PAIR_TIMEOUT_MS: u64 = 15_000;
    const POLLS_PER_TICK: usize = 3;
    const FLUSH_MS: u64 = 300;

    #[gatt_server]
    struct Server {
        svc: CatSvc,
    }

    #[gatt_service(uuid = "c3321c00-836b-0fa2-914c-4b7d1a5ec332")]
    struct CatSvc {
        #[characteristic(uuid = "c3321c01-836b-0fa2-914c-4b7d1a5ec332", write_without_response)]
        rx: [u8; 20],
        #[characteristic(uuid = "c3321c02-836b-0fa2-914c-4b7d1a5ec332", notify)]
        tx: [u8; 20],
    }

    struct Handler<'a>(&'a RefCell<Shared>);

    impl EventHandler for Handler<'_> {
        fn on_adv_reports(&self, reports: bt_hci::param::LeAdvReportsIter) {
            let mut s = self.0.borrow_mut();
            for r in reports.filter_map(|r| r.ok()) {
                if let Some((id, seq, len)) = s.ingest_advert(r.data) {
                    println!("[BLE] heard {:04x} seq={} payload={}B", id, seq, len);
                }
            }
        }
    }

    type Ctrl = bt_hci::controller::ExternalController<BleConnector<'static>, 4>;

    type Driver = Pin<Box<dyn Future<Output = ()>>>;

    pub struct BleManager {
        sh: Rc<RefCell<Shared>>,
        driver: Option<Driver>,
    }

    impl BleManager {
        pub fn new() -> Self {
            let mac = esp_hal::efuse::base_mac_address();
            let own = ble_proto::id_from_mac(mac.as_bytes().try_into().unwrap_or([0; 6]));
            Self {
                sh: Rc::new(RefCell::new(Shared::new(own))),
                driver: None,
            }
        }

        /// Start the BLE stack on an initialised controller.
        pub fn attach(&mut self, connector: BleConnector<'static>) {
            let controller: bt_hci::controller::ExternalController<_, 4> =
                bt_hci::controller::ExternalController::new(connector);
            self.driver = Some(Box::pin(drive(controller, self.sh.clone())));
        }

        /// Stop the stack. Queued frames get a short grace period to go out
        /// (e.g. the visit-bye), then the stack is dropped, powering the
        /// radio down.
        pub fn detach(&mut self) {
            let start = Instant::now();
            while self.driver.is_some()
                && self.has_pending()
                && start.elapsed() < Duration::from_millis(FLUSH_MS)
            {
                self.pump();
                esp_hal::delay::Delay::new().delay_millis(10);
            }
            self.driver = None;
            println!("[BLE] detached");
            let mut s = self.sh.borrow_mut();
            s.clear_outbox();
            s.want = None;
            s.link = None;
            s.pair_code = None;
            s.pair_answer = None;
        }

        fn has_pending(&self) -> bool {
            let s = self.sh.borrow();
            s.link.is_some() && s.has_outbox(s.link.unwrap_or(0))
        }

        fn pump(&mut self) {
            let Some(d) = self.driver.as_mut() else {
                return;
            };
            let mut cx = Context::from_waker(Waker::noop());
            if let Poll::Ready(()) = d.as_mut().poll(&mut cx) {
                println!("[BLE] driver ended");
                self.driver = None;
            }
        }
    }

    impl Transport for BleManager {
        fn own_id(&self) -> Option<PeerId> {
            Some(PeerId::from_short(self.sh.borrow().own))
        }
        fn is_active(&self) -> bool {
            self.sh.borrow().active
        }
        fn activate(&mut self) {
            self.sh.borrow_mut().active = true;
        }
        fn deactivate(&mut self) {
            self.sh.borrow_mut().active = false;
        }
        fn add_peer(&mut self, _peer: PeerId) {}
        fn send_broadcast(&mut self, data: &[u8]) {
            if !self.sh.borrow_mut().set_broadcast(data) {
                println!("[BLE] broadcast too long ({})", data.len());
            }
        }
        fn send_to(&mut self, peer: PeerId, data: &[u8]) {
            if peer == PeerId::BROADCAST {
                self.send_broadcast(data);
            } else if !self.sh.borrow_mut().queue(ble_proto::short_id(peer), data) {
                println!("[BLE] frame too long ({})", data.len());
            }
        }
        fn poll(&mut self) {
            for _ in 0..POLLS_PER_TICK {
                self.pump();
            }
        }
        fn drain(&mut self) -> Vec<InboxItem, INBOX_CAPACITY> {
            self.sh.borrow_mut().drain()
        }
        fn pairing_code(&self) -> Option<u32> {
            self.sh.borrow().pair_code
        }
        fn answer_pairing(&mut self, accept: bool) {
            self.sh.borrow_mut().pair_answer = Some(accept);
        }
    }

    /// Show `code` to the player and wait for their yes/no. Times out as a
    /// rejection so an unattended cat never stays half-paired.
    async fn await_pair_decision(sh: &RefCell<Shared>, code: u32) -> bool {
        {
            let mut s = sh.borrow_mut();
            s.pair_code = Some(code);
            s.pair_answer = None;
        }
        let start = Instant::now();
        let accept = loop {
            if let Some(a) = sh.borrow().pair_answer {
                break a;
            }
            if start.elapsed() >= Duration::from_millis(PAIR_TIMEOUT_MS) {
                break false;
            }
            Timer::after_millis(50).await;
        };
        let mut s = sh.borrow_mut();
        s.pair_code = None;
        s.pair_answer = None;
        accept
    }

    /// The GAP service build uses write-once statics inside trouble-host, so
    /// the server can only be constructed once per boot; later sessions reuse it.
    fn gatt_server() -> Option<&'static Server<'static>> {
        static mut SERVER: Option<Server<'static>> = None;
        // SAFETY: only the single game-loop thread builds or reads the server.
        unsafe {
            let slot = &mut *core::ptr::addr_of_mut!(SERVER);
            if slot.is_none() {
                *slot = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
                    name: "catode32",
                    appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
                }))
                .ok();
            }
            slot.as_ref()
        }
    }

    const BOND_LEN: usize = 40;
    const MAX_BONDS: usize = 6;

    fn encode_bond(b: &BondInformation, out: &mut [u8]) -> bool {
        if b.identity.addr.kind != AddrKind::RANDOM || !b.is_bonded {
            return false;
        }
        let level = match b.security_level {
            SecurityLevel::EncryptedAuthenticated => 2,
            SecurityLevel::Encrypted => 1,
            _ => return false,
        };
        out[..6].copy_from_slice(b.identity.addr.addr.raw());
        out[6..22].copy_from_slice(&b.ltk.0.to_le_bytes());
        let irk = b.identity.irk.map_or(0, |k| k.0.get());
        out[22..38].copy_from_slice(&irk.to_le_bytes());
        out[38] = level;
        out[39] = 1;
        true
    }

    fn decode_bond(r: &[u8]) -> Option<BondInformation> {
        let addr: [u8; 6] = r[..6].try_into().ok()?;
        let ltk = u128::from_le_bytes(r[6..22].try_into().ok()?);
        let irk = u128::from_le_bytes(r[22..38].try_into().ok()?);
        let level = match r[38] {
            2 => SecurityLevel::EncryptedAuthenticated,
            1 => SecurityLevel::Encrypted,
            _ => return None,
        };
        let identity = Identity {
            addr: Address::random(addr),
            irk: IdentityResolvingKey::new(irk),
        };
        Some(BondInformation::new(identity, LongTermKey(ltk), level, r[39] != 0))
    }

    /// Bonds persisted in flash: `[count][BOND_LEN * count]`.
    fn load_bonds() -> Vec<BondInformation, MAX_BONDS> {
        let mut out = Vec::new();
        let mut buf = [0u8; 1 + BOND_LEN * MAX_BONDS];
        let Some(n) = crate::storage::read_bonds(&mut buf) else {
            return out;
        };
        if n == 0 {
            return out;
        }
        let count = (buf[0] as usize).min(MAX_BONDS);
        for i in 0..count {
            let s = 1 + i * BOND_LEN;
            if s + BOND_LEN > n {
                break;
            }
            if let Some(b) = decode_bond(&buf[s..s + BOND_LEN]) {
                let _ = out.push(b);
            }
        }
        out
    }

    /// Add (or replace) a bond in flash, evicting the oldest past the cap.
    fn persist_bond(bond: &BondInformation) {
        let mut rec = [0u8; BOND_LEN];
        if !encode_bond(bond, &mut rec) {
            return;
        }
        let mut list = load_bonds();
        list.retain(|b| b.identity.addr != bond.identity.addr);
        if list.is_full() {
            list.remove(0);
        }
        let _ = list.push(bond.clone());
        let mut buf = [0u8; 1 + BOND_LEN * MAX_BONDS];
        let mut n = 0;
        for b in list.iter() {
            let s = 1 + n * BOND_LEN;
            if encode_bond(b, &mut buf[s..s + BOND_LEN]) {
                n += 1;
            }
        }
        buf[0] = n as u8;
        let ok = crate::storage::write_bonds(&buf[..1 + n * BOND_LEN]);
        println!("[BLE] bond persisted ok={} total={}", ok, n);
    }

    /// Fail-fast GATT client creations in a row (see `central_visit`).
    static CLIENT_FAILS: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

    /// Drop a peer's bond from flash after the peer reports it lost its own.
    /// A fresh pairing re-adds it via `persist_bond`.
    fn forget_bond(addr: &Address) {
        let mut list = load_bonds();
        let before = list.len();
        list.retain(|b| b.identity.addr != *addr);
        if list.len() == before {
            return;
        }
        let mut buf = [0u8; 1 + BOND_LEN * MAX_BONDS];
        let mut n = 0;
        for b in list.iter() {
            let s = 1 + n * BOND_LEN;
            if encode_bond(b, &mut buf[s..s + BOND_LEN]) {
                n += 1;
            }
        }
        buf[0] = n as u8;
        let ok = crate::storage::write_bonds(&buf[..1 + n * BOND_LEN]);
        println!("[BLE] bond forgotten ok={} total={}", ok, n);
    }

    async fn drive(controller: Ctrl, sh: Rc<RefCell<Shared>>) {
        let own = sh.borrow().own;
        let address = Address::random(ble_proto::address_for(own));
        let mut resources: HostResources<Ctrl, DefaultPacketPool, 1, 3> = HostResources::new();
        let stack = trouble_host::new(controller, &mut resources)
            .set_random_address(address)
            .set_io_capabilities(IoCapabilities::DisplayYesNo)
            .build();
        for b in load_bonds() {
            let _ = stack.add_bond_information(b);
        }
        let mut runner = stack.runner();
        let mut peripheral = stack.peripheral();
        let mut central = stack.central();
        let Some(server) = gatt_server() else {
            println!("[BLE] gatt server init failed");
            return;
        };
        let handler = Handler(&sh);
        println!("[BLE] stack up, id={:04x}", own);
        let work = async {
            loop {
                let want = sh.borrow().want;
                if let Some(target) = want {
                    println!("[BLE] visit -> {:04x}", target);
                    central_visit(&stack, &mut central, &sh, target).await;
                    {
                        let mut s = sh.borrow_mut();
                        s.pair_code = None;
                        s.pair_answer = None;
                    }
                    println!("[BLE] visit ended {:04x}", target);
                    let mut s = sh.borrow_mut();
                    s.want = None;
                    s.link = None;
                    s.drop_outbox(target);
                    continue;
                }
                advertise_phase(&mut peripheral, &server, &sh).await;
                if sh.borrow().want.is_some() {
                    continue;
                }
                let mut scanner = Scanner::new(central);
                let cfg = ScanConfig {
                    active: false,
                    interval: Duration::from_millis(100),
                    window: Duration::from_millis(100),
                    ..Default::default()
                };
                if let Ok(session) = scanner.scan(&cfg).await {
                    Timer::after_millis(SCAN_MS).await;
                    drop(session);
                } else {
                    println!("[BLE] scan start failed");
                }
                central = scanner.into_inner();
            }
        };

        let _ = join(runner.run_with_handler(&handler), work).await;
    }

    async fn advertise_phase(
        peripheral: &mut Peripheral<'_, Ctrl, DefaultPacketPool>,
        server: &Server<'_>,
        sh: &RefCell<Shared>,
    ) {
        let mut ad = [0u8; ble_proto::ADVERT_MAX];
        let n = sh.borrow().advert(&mut ad);
        // Scan response: complete local name + 128-bit service UUID, so
        // phone scanners can find the device by name or UUID.
        let mut scan = [0u8; 31];
        let mut name = crate::t!("BLE name");
        while name.len() > 11 {
            name = &name[..name.floor_char_boundary(name.len() - 1)];
        }
        let nb = name.as_bytes();
        scan[0] = nb.len() as u8 + 1;
        scan[1] = 0x09;
        scan[2..2 + nb.len()].copy_from_slice(nb);
        let u = 2 + nb.len();
        scan[u] = 17;
        scan[u + 1] = 0x07;
        scan[u + 2..u + 18].copy_from_slice(&SERVICE_UUID);
        let scan_len = u + 18;
        let Ok(advertiser) = peripheral
            .advertise(
                &Default::default(),
                Advertisement::ConnectableScannableUndirected {
                    adv_data: &ad[..n],
                    scan_data: &scan[..scan_len],
                },
            )
            .await
        else {
            println!("[BLE] advertise start failed");
            return;
        };
        println!("[BLE] advertising {}B", n);
        let accepted = match select(advertiser.accept(), Timer::after_millis(ADVERTISE_MS)).await {
            Either::First(Ok(a)) => a,
            _ => return,
        };
        let Ok(conn) = accepted.with_attribute_server(server) else {
            return;
        };
        let Some(peer) = ble_proto::id_from_address(conn.raw().peer_address().addr.raw()) else {
            return;
        };
        println!("[BLE] connected from {:04x}", peer);
        sh.borrow_mut().link = Some(peer);
        peripheral_session(server, &conn, sh, peer).await;
        {
            let mut s = sh.borrow_mut();
            s.pair_code = None;
            s.pair_answer = None;
        }
        println!("[BLE] session ended {:04x}", peer);
        let mut s = sh.borrow_mut();
        s.link = None;
        s.drop_outbox(peer);
    }

    async fn peripheral_session<P: PacketPool>(
        server: &Server<'_>,
        conn: &GattConnection<'_, '_, P>,
        sh: &RefCell<Shared>,
        peer: u16,
    ) {
        let rx = server.svc.rx.handle;
        let _ = conn.raw().set_bondable(true);
        let events = async {
            loop {
                match conn.next().await {
                    GattConnectionEvent::Disconnected { .. } => break,
                    GattConnectionEvent::PassKeyConfirm(k) => {
                        println!("[BLE] peripheral passkey {}", k.value());
                        if await_pair_decision(sh, k.value()).await {
                            let _ = conn.pass_key_confirm();
                        } else {
                            let _ = conn.pass_key_cancel();
                        }
                    }
                    GattConnectionEvent::PairingComplete { security_level, bond } => {
                        println!("[BLE] peripheral paired {:?} bond={}", security_level, bond.is_some());
                        if let Some(b) = &bond {
                            persist_bond(b);
                        }
                    }
                    GattConnectionEvent::PairingFailed(e) => {
                        println!("[BLE] peripheral pairing failed {:?}", e);
                        break;
                    }
                    GattConnectionEvent::BondLost => {
                        println!("[BLE] peripheral: peer lost bond");
                        forget_bond(&conn.raw().peer_identity().addr);
                    }
                    GattConnectionEvent::Gatt { event } => {
                        let secure = matches!(
                            conn.raw().security_level(),
                            Ok(SecurityLevel::EncryptedAuthenticated)
                        );
                        if !secure {
                            println!("[BLE] peripheral rejected unsecured GATT access");
                            if let Ok(reply) = event.reject(AttErrorCode::INSUFFICIENT_AUTHENTICATION) {
                                reply.send().await;
                            }
                            continue;
                        }
                        if let GattEvent::Write(w) = &event {
                            if w.handle() == rx {
                                w.with_data(|_, d| sh.borrow_mut().push_inbox(peer, d));
                            }
                        }
                        if let Ok(reply) = event.accept() {
                            reply.send().await;
                        }
                    }
                    _ => {}
                }
            }
        };
        let sender = async {
            loop {
                if !matches!(conn.raw().security_level(), Ok(SecurityLevel::EncryptedAuthenticated)) {
                    Timer::after_millis(50).await;
                    continue;
                }
                let next = sh.borrow_mut().take_outbox(peer);
                match next {
                    Some(f) => {
                        if server.svc.tx.notify_raw(conn, &f, false).await.is_err() {
                            break;
                        }
                    }
                    None => Timer::after_millis(20).await,
                }
            }
        };
        select(events, sender).await;
    }

    async fn central_visit(
        stack: &Stack<'_, Ctrl, DefaultPacketPool>,
        central: &mut Central<'_, Ctrl, DefaultPacketPool>,
        sh: &RefCell<Shared>,
        target: u16,
    ) {
        let addr = Address::random(ble_proto::address_for(target));
        let cfg = ConnectConfig {
            connect_params: Default::default(),
            scan_config: ScanConfig {
                filter_accept_list: &[addr],
                ..Default::default()
            },
        };
        let conn = match select(central.connect(&cfg), Timer::after_millis(CONNECT_TIMEOUT_MS)).await {
            Either::First(Ok(c)) => c,
            _ => {
                println!("[BLE] connect to {:04x} failed", target);
                return;
            }
        };
        let Ok(client) = GattClient::<Ctrl, DefaultPacketPool, 10>::new(stack, &conn).await else {
            println!("[BLE] gatt client failed");
            // A peer that dropped its bond rejects our stored key and hangs up
            // before any event reaches us; clear the stale bond after repeats.
            if CLIENT_FAILS.fetch_add(1, core::sync::atomic::Ordering::Relaxed) >= 1 {
                CLIENT_FAILS.store(0, core::sync::atomic::Ordering::Relaxed);
                let id = conn.peer_identity();
                let _ = stack.remove_bond_information(id);
                forget_bond(&id.addr);
            }
            return;
        };
        CLIENT_FAILS.store(0, core::sync::atomic::Ordering::Relaxed);
        println!("[BLE] connected to {:04x}", target);
        sh.borrow_mut().link = Some(target);

        let session = async {
            // Hold GATT traffic until pairing finishes; the player may still be
            // reading the code.
            let wait_start = Instant::now();
            while !matches!(conn.security_level(), Ok(SecurityLevel::EncryptedAuthenticated)) {
                if wait_start.elapsed() >= Duration::from_millis(PAIR_TIMEOUT_MS + 3_000) || !conn.is_connected() {
                    println!("[BLE] central not secured, giving up");
                    return;
                }
                Timer::after_millis(50).await;
            }
            let Ok(services) = client.services_by_uuid(&Uuid::new_long(SERVICE_UUID)).await else {
                return;
            };
            let Some(service) = services.first().cloned() else {
                return;
            };
            let Ok(rx) = client
                .characteristic_by_uuid::<[u8; GATT_PAYLOAD_MAX]>(&service, &Uuid::new_long(RX_CHAR_UUID))
                .await
            else {
                return;
            };
            let Ok(tx) = client
                .characteristic_by_uuid::<[u8; GATT_PAYLOAD_MAX]>(&service, &Uuid::new_long(TX_CHAR_UUID))
                .await
            else {
                return;
            };
            let Ok(mut listener) = client.subscribe(&tx, false).await else {
                return;
            };
            let mut last = Instant::now();
            loop {
                let out = sh.borrow_mut().take_outbox(target);
                if let Some(f) = out {
                    if client.write_characteristic_without_response(&rx, &f).await.is_err() {
                        return;
                    }
                    last = Instant::now();
                    continue;
                }
                match select(listener.next(), Timer::after_millis(20)).await {
                    Either::First(n) => {
                        sh.borrow_mut().push_inbox(target, n.as_ref());
                        last = Instant::now();
                    }
                    _ => {
                        if last.elapsed() >= Duration::from_millis(LINK_IDLE_MS) || !conn.is_connected() {
                            return;
                        }
                    }
                }
            }
        };
        let _ = conn.set_bondable(true);
        let _ = conn.request_security();
        let events = async {
            loop {
                match conn.next().await {
                    ConnectionEvent::Disconnected { .. } => break,
                    ConnectionEvent::PassKeyConfirm(k) => {
                        println!("[BLE] central passkey {}", k.value());
                        if await_pair_decision(sh, k.value()).await {
                            let _ = conn.pass_key_confirm();
                        } else {
                            let _ = conn.pass_key_cancel();
                        }
                    }
                    ConnectionEvent::PairingComplete { security_level, bond } => {
                        println!("[BLE] central paired {:?} bond={}", security_level, bond.is_some());
                        if let Some(b) = &bond {
                            persist_bond(b);
                        }
                    }
                    ConnectionEvent::PairingFailed(e) => {
                        println!("[BLE] central pairing failed {:?}", e);
                        break;
                    }
                    ConnectionEvent::BondLost => {
                        println!("[BLE] central: peer lost bond");
                        forget_bond(&conn.peer_identity().addr);
                    }
                    _ => {}
                }
            }
        };
        let _ = select(select(client.task(), session), events).await;
    }
}
