//! The GATT table: HID over GATT (a keyboard and a consumer control),
//! plus the Battery and Device Information services HID hosts expect.

// Battery, Device Information and most HID characteristics are only read
// by the phone; the host code never touches them after registering, which
// the derive macros' fields would otherwise warn about.
#![allow(dead_code)]

use trouble_host::prelude::*;

use super::keys::{CONSUMER_REPORT_ID, KEYBOARD_REPORT_ID, REPORT_MAP, REPORT_MAP_LEN};

/// HID Information: bcdHID 1.11, no country, normally connectable.
const HID_INFO: [u8; 4] = [0x11, 0x01, 0x00, 0x02];
/// PnP ID: USB vendor ID source, pid.codes vendor 0x1209, product 0x0001
/// (their test ID), version 1.0.
const PNP_ID: [u8; 7] = [0x02, 0x09, 0x12, 0x01, 0x00, 0x00, 0x01];

#[gatt_server]
pub(super) struct Server {
    pub(super) hid: HidService,
    battery: BatteryService,
    device_info: DeviceInfoService,
}

#[gatt_service(uuid = service::HUMAN_INTERFACE_DEVICE)]
pub(super) struct HidService {
    #[characteristic(uuid = "2a4a", read, value = HID_INFO, permissions(encrypted))]
    hid_info: [u8; 4],
    #[characteristic(uuid = "2a4b", read, value = REPORT_MAP, permissions(encrypted))]
    report_map: [u8; REPORT_MAP_LEN],
    #[characteristic(uuid = "2a4c", write_without_response, permissions(encrypted))]
    control_point: u8,
    #[characteristic(uuid = "2a4e", read, write_without_response, value = 1, permissions(encrypted))]
    protocol_mode: u8,
    #[descriptor(uuid = "2908", read = encrypted, value = [KEYBOARD_REPORT_ID, 1])]
    #[characteristic(uuid = "2a4d", read, notify, permissions(encrypted))]
    pub(super) keyboard: [u8; 8],
    #[descriptor(uuid = "2908", read = encrypted, value = [CONSUMER_REPORT_ID, 1])]
    #[characteristic(uuid = "2a4d", read, notify, permissions(encrypted))]
    pub(super) consumer: [u8; 2],
}

/// There's no battery measurement; HID hosts expect the service anyway.
#[gatt_service(uuid = service::BATTERY)]
pub(super) struct BatteryService {
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify, value = 100)]
    level: u8,
}

#[gatt_service(uuid = service::DEVICE_INFORMATION)]
pub(super) struct DeviceInfoService {
    #[characteristic(uuid = characteristic::PNP_ID, read, value = PNP_ID)]
    pnp_id: [u8; 7],
    #[characteristic(uuid = characteristic::MANUFACTURER_NAME_STRING, read, value = *b"catode32")]
    manufacturer: [u8; 8],
}

