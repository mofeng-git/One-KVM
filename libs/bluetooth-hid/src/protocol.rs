//! Classic HIDP framing and report descriptor.
use crate::Report;
pub const MAP: &[u8] = &[
    0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x85, 0x01, 0x05, 0x07, 0x19, 0xe0, 0x29, 0xe7, 0x15, 0x00,
    0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x95, 0x01, 0x75, 0x08, 0x81, 0x01, 0x95, 0x05,
    0x75, 0x01, 0x05, 0x08, 0x19, 0x01, 0x29, 0x05, 0x91, 0x02, 0x95, 0x01, 0x75, 0x03, 0x91, 0x01,
    0x95, 0x06, 0x75, 0x08, 0x15, 0x00, 0x25, 0x65, 0x05, 0x07, 0x19, 0x00, 0x29, 0x65, 0x81, 0x00,
    0xc0, 0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x85, 0x02, 0x09, 0x01, 0xa1, 0x00, 0x05, 0x09, 0x19,
    0x01, 0x29, 0x05, 0x15, 0x00, 0x25, 0x01, 0x95, 0x05, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01, 0x75,
    0x03, 0x81, 0x01, 0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x38, 0x15, 0x81, 0x25, 0x7f, 0x75,
    0x08, 0x95, 0x03, 0x81, 0x06, 0xc0, 0xc0, 0x05, 0x0c, 0x09, 0x01, 0xa1, 0x01, 0x85, 0x03, 0x15,
    0x00, 0x26, 0xff, 0x03, 0x19, 0x00, 0x2a, 0xff, 0x03, 0x75, 0x10, 0x95, 0x01, 0x81, 0x00, 0xc0,
];

pub const HID_UUID: &str = "00001124-0000-1000-8000-00805f9b34fb";
pub fn sdp() -> String {
    let hex: String = MAP.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        r#"<?xml version="1.0"?>
<record>
  <attribute id="0x0001"><sequence><uuid value="0x1124"/></sequence></attribute>
  <attribute id="0x0004"><sequence><sequence><uuid value="0x0100"/><uint16 value="0x0011"/></sequence><sequence><uuid value="0x0011"/></sequence></sequence></attribute>
  <attribute id="0x0005"><sequence><uuid value="0x1002"/></sequence></attribute>
  <attribute id="0x0006"><sequence><uint16 value="0x656e"/><uint16 value="0x006a"/><uint16 value="0x0100"/></sequence></attribute>
  <attribute id="0x0009"><sequence><sequence><uuid value="0x1124"/><uint16 value="0x0100"/></sequence></sequence></attribute>
  <attribute id="0x000d"><sequence><sequence><sequence><uuid value="0x0100"/><uint16 value="0x0013"/></sequence><sequence><uuid value="0x0011"/></sequence></sequence></sequence></attribute>
  <attribute id="0x0100"><text value="One-KVM Keyboard and Mouse"/></attribute>
  <attribute id="0x0101"><text value="Classic Bluetooth HID"/></attribute>
  <attribute id="0x0102"><text value="One-KVM"/></attribute>
  <attribute id="0x0200"><uint16 value="0x0100"/></attribute>
  <attribute id="0x0201"><uint16 value="0x0111"/></attribute>
  <attribute id="0x0202"><uint8 value="0xc0"/></attribute>
  <attribute id="0x0203"><uint8 value="0x00"/></attribute>
  <attribute id="0x0204"><boolean value="false"/></attribute>
  <attribute id="0x0205"><boolean value="false"/></attribute>
  <attribute id="0x0206"><sequence><sequence><uint8 value="0x22"/><text encoding="hex" value="{hex}"/></sequence></sequence></attribute>
  <attribute id="0x0207"><sequence><sequence><uint16 value="0x0409"/><uint16 value="0x0100"/></sequence></sequence></attribute>
  <attribute id="0x0209"><boolean value="false"/></attribute>
  <attribute id="0x020a"><boolean value="false"/></attribute>
  <attribute id="0x020b"><uint16 value="0x0100"/></attribute>
  <attribute id="0x020c"><uint16 value="0x0c80"/></attribute>
  <attribute id="0x020d"><boolean value="true"/></attribute>
  <attribute id="0x020e"><boolean value="true"/></attribute>
</record>"#
    )
}
#[derive(Debug, Default)]
pub struct HidProtocol {
    pub boot: bool,
    pub suspended: bool,
    pub leds: u8,
    pub keyboard: [u8; 8],
    pub buttons: u8,
    pub consumer: [u8; 2],
}
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ControlResult {
    pub reply: Option<Vec<u8>>,
    pub unplug: bool,
    pub reset: bool,
}
impl HidProtocol {
    pub fn input(&mut self, kind: Report, value: &[u8]) -> Result<Vec<u8>, String> {
        if value.len() != kind.len() {
            return Err("Invalid HID report length".into());
        }
        if self.suspended {
            return Err("Bluetooth HID is suspended".into());
        }
        let mut packet = vec![0xa1, kind.id()];
        match kind {
            Report::Keyboard => {
                self.keyboard.copy_from_slice(value);
                packet.extend_from_slice(value);
            }
            Report::Mouse => {
                self.buttons = value[0];
                packet.extend_from_slice(&value[..if self.boot { 3 } else { 4 }]);
                if self.boot {
                    packet[2] &= 7;
                }
            }
            Report::Consumer if !self.boot => {
                self.consumer.copy_from_slice(value);
                packet.extend_from_slice(value);
            }
            Report::Consumer => return Err("Consumer keys unavailable in boot protocol".into()),
        }
        Ok(packet)
    }
    pub fn release(&mut self) -> Vec<Vec<u8>> {
        self.keyboard = [0; 8];
        self.buttons = 0;
        self.consumer = [0; 2];
        let mut packets = vec![
            vec![0xa1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            if self.boot {
                vec![0xa1, 2, 0, 0, 0]
            } else {
                vec![0xa1, 2, 0, 0, 0, 0]
            },
        ];
        if !self.boot {
            packets.push(vec![0xa1, 3, 0, 0]);
        }
        packets
    }
    pub fn output(&mut self, data: &[u8]) -> bool {
        if data.len() == 3 && data[0] == 0xa2 && data[1] == 1 {
            self.leds = data[2] & 0x1f;
            true
        } else {
            false
        }
    }
    pub fn control(&mut self, data: &[u8]) -> ControlResult {
        let mut result = ControlResult::default();
        let handshake = |code| ControlResult {
            reply: Some(vec![code]),
            ..Default::default()
        };
        let Some(&header) = data.first() else {
            return handshake(4);
        };
        match header {
            0x13 if data.len() == 1 => {
                self.suspended = true;
                result.reset = true;
            }
            0x14 if data.len() == 1 => {
                self.suspended = false;
                result.reset = true;
            }
            0x15 if data.len() == 1 => {
                result.unplug = true;
            }
            0x11 | 0x12 if data.len() == 1 => {
                self.suspended = false;
                result.reset = true;
            }
            0x60 if data.len() == 1 => {
                result.reply = Some(vec![0xa0, if self.boot { 0 } else { 1 }]);
            }
            0x70 | 0x71 if data.len() == 1 => {
                self.boot = header == 0x70;
                result.reply = Some(vec![0]);
                result.reset = true;
            }
            0x52 if data.len() == 3 && data[1] == 1 => {
                self.leds = data[2] & 0x1f;
                return handshake(0);
            }
            0x41 | 0x49 | 0x42 | 0x4a => {
                let sized = header & 8 != 0;
                if data.len() != if sized { 4 } else { 2 } {
                    return handshake(4);
                }
                let mut report = vec![0xa0 | (header & 3), data[1]];
                match (header & 3, data[1]) {
                    (1, 1) => report.extend(self.keyboard),
                    (1, 2) => report.extend(if self.boot {
                        vec![self.buttons & 7, 0, 0]
                    } else {
                        vec![self.buttons, 0, 0, 0]
                    }),
                    (1, 3) if !self.boot => report.extend(self.consumer),
                    (2, 1) => report.push(self.leds),
                    _ => return handshake(2),
                }
                if sized {
                    report.truncate(1 + u16::from_le_bytes([data[2], data[3]]) as usize);
                }
                result.reply = Some(report);
            }
            _ => return handshake(3), // ERR_UNSUPPORTED_REQUEST
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_frames_and_boot_protocol() {
        let mut hid = HidProtocol::default();
        assert_eq!(
            hid.input(Report::Keyboard, &[2, 0, 4, 0, 0, 0, 0, 0])
                .unwrap(),
            [0xa1, 1, 2, 0, 4, 0, 0, 0, 0, 0]
        );
        assert_eq!(hid.control(&[0x60]).reply.unwrap(), [0xa0, 1]);
        assert_eq!(hid.control(&[0x70]).reply.unwrap(), [0]);
        assert_eq!(
            hid.input(Report::Mouse, &[0x1f, 127, 128, 1]).unwrap(),
            [0xa1, 2, 7, 127, 128]
        );
        assert!(hid.input(Report::Consumer, &[0, 0]).is_err());
        assert!(hid.input(Report::Keyboard, &[0]).is_err());
    }
    #[test]
    fn led_read_write_and_errors() {
        let mut hid = HidProtocol::default();
        assert!(hid.output(&[0xa2, 1, 3]));
        assert_eq!(hid.leds, 3);
        assert_eq!(hid.control(&[0x42, 1]).reply.unwrap(), [0xa2, 1, 3]);
        assert_eq!(hid.control(&[0x52, 1, 2]).reply.unwrap(), [0]);
        assert_eq!(hid.leds, 2);
        assert_eq!(hid.control(&[0x41, 99]).reply.unwrap(), [2]);
        assert_eq!(hid.control(&[0x41]).reply.unwrap(), [4]);
        assert_eq!(hid.control(&[0x90, 0]).reply.unwrap(), [3]);
        assert_eq!(
            hid.control(&[0x49, 1, 3, 0]).reply.unwrap(),
            [0xa1, 1, 0, 0]
        );
    }
    #[test]
    fn suspend_reset_and_virtual_unplug() {
        let mut hid = HidProtocol::default();
        assert!(hid.control(&[0x13]).reset);
        assert!(hid.input(Report::Mouse, &[0; 4]).is_err());
        assert!(hid.control(&[0x14]).reset);
        assert!(hid.input(Report::Mouse, &[0; 4]).is_ok());
        assert!(hid.control(&[0x15]).unplug);
        hid.keyboard[2] = 4;
        hid.buttons = 1;
        assert_eq!(hid.release().len(), 3);
        assert_eq!(hid.keyboard, [0; 8]);
        assert_eq!(hid.buttons, 0);
    }
    #[test]
    fn sdp_describes_both_classic_channels() {
        let record = sdp();
        assert!(record.contains("uuid value=\"0x1124\""));
        assert!(record.contains("uint16 value=\"0x0011\""));
        assert!(record.contains("uint16 value=\"0x0013\""));
        assert!(!record.contains("0x1812"));
    }
    #[test]
    fn descriptor_report_sizes() {
        let (mut pos, mut size, mut count, mut id, mut depth) = (0, 0, 0, 0, 0);
        let mut input = [0; 4];
        let mut output = [0; 4];
        while pos < MAP.len() {
            let prefix = MAP[pos];
            pos += 1;
            let length = [0, 1, 2, 4][(prefix & 3) as usize];
            let mut value = 0;
            for i in 0..length {
                value |= (MAP[pos + i] as usize) << (8 * i);
            }
            pos += length;
            match ((prefix >> 2) & 3, prefix >> 4) {
                (1, 7) => size = value,
                (1, 8) => id = value,
                (1, 9) => count = value,
                (0, 8) => input[id] += size * count,
                (0, 9) => output[id] += size * count,
                (0, 10) => depth += 1,
                (0, 12) => depth -= 1,
                _ => {}
            }
        }
        assert_eq!(depth, 0);
        assert_eq!(input, [0, 64, 32, 16]);
        assert_eq!(output, [0, 8, 0, 0]);
    }
}
