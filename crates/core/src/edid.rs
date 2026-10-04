//! Just enough of EDID to recognise a monitor: maker, product, serial, name.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edid {
    pub vendor: String,
    pub product: String,
    pub serial: String,
    pub name: String,
}

const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];

/// The text of the first display descriptor with this tag (0xff serial,
/// 0xfc name).
fn descriptor_text(bytes: &[u8], tag: u8) -> Option<String> {
    (0..4).find_map(|slot| {
        let at = 54 + slot * 18;
        let d = &bytes[at..at + 18];
        (d[0] == 0 && d[1] == 0 && d[2] == 0 && d[3] == tag).then(|| {
            let text: String = d[5..18]
                .iter()
                .take_while(|c| **c != 0x0a)
                .map(|c| *c as char)
                .collect();
            text.trim().to_string()
        })
    })
}

pub fn parse_edid(bytes: &[u8]) -> Option<Edid> {
    if bytes.len() < 128 || bytes[..8] != HEADER {
        return None;
    }
    let id = u16::from_be_bytes([bytes[8], bytes[9]]);
    let letter = |shift: u16| (((id >> shift) & 0x1f) as u8 + b'A' - 1) as char;
    let vendor: String = [letter(10), letter(5), letter(0)].iter().collect();
    let product = format!("0x{:04x}", u16::from_le_bytes([bytes[10], bytes[11]]));
    let number = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let serial = descriptor_text(bytes, 0xff)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if number == 0 {
                String::new()
            } else {
                number.to_string()
            }
        });
    Some(Edid {
        vendor,
        product,
        serial,
        name: descriptor_text(bytes, 0xfc).unwrap_or_default(),
    })
}

/// A 128-byte EDID base block with the given identity, for tests.
#[cfg(test)]
pub(crate) fn build(
    vendor: &str,
    product: u16,
    serial_number: u32,
    serial_text: Option<&str>,
    name: &str,
) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[..8].copy_from_slice(&[0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
    let letters: Vec<u16> = vendor.bytes().map(|c| (c - b'A' + 1) as u16).collect();
    let id = (letters[0] << 10) | (letters[1] << 5) | letters[2];
    b[8..10].copy_from_slice(&id.to_be_bytes());
    b[10..12].copy_from_slice(&product.to_le_bytes());
    b[12..16].copy_from_slice(&serial_number.to_le_bytes());
    let mut put = |slot: usize, tag: u8, text: &str| {
        let at = 54 + slot * 18;
        b[at + 3] = tag;
        let mut field = [b' '; 13];
        let bytes = text.as_bytes();
        field[..bytes.len()].copy_from_slice(bytes);
        if bytes.len() < 13 {
            field[bytes.len()] = 0x0a;
        }
        b[at + 5..at + 18].copy_from_slice(&field);
    };
    put(1, 0xfc, name);
    if let Some(s) = serial_text {
        put(2, 0xff, s);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_maker_product_serial_and_name() {
        let e = parse_edid(&build("DEL", 0x41b5, 0, Some("ABC123"), "DELL U2723QE")).unwrap();
        assert_eq!(
            e,
            Edid {
                vendor: "DEL".into(),
                product: "0x41b5".into(),
                serial: "ABC123".into(),
                name: "DELL U2723QE".into(),
            }
        );
    }

    #[test]
    fn falls_back_to_the_numeric_serial() {
        let e = parse_edid(&build("BOE", 0x095f, 12_345_678, None, "")).unwrap();
        assert_eq!(e.serial, "12345678");
        assert_eq!(e.name, "");
        let none = parse_edid(&build("BOE", 0x095f, 0, None, "")).unwrap();
        assert_eq!(none.serial, "");
    }

    #[test]
    fn rejects_what_is_not_edid() {
        assert_eq!(parse_edid(&build("DEL", 1, 0, None, "x")[..64]), None);
        let mut bad = build("DEL", 1, 0, None, "x");
        bad[0] = 0x12;
        assert_eq!(parse_edid(&bad), None);
    }
}
