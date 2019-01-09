const BASIC_DATA_PARTITION: &[u8; 16] = &[
    0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44, 0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99, 0xC7,
];

pub trait AttributeBits {
    fn display_attribute_bits(&self, type_guid: [u8; 16]) -> String;
}

impl AttributeBits for u64 {
    fn display_attribute_bits(&self, type_guid: [u8; 16]) -> String {
        let mut attributes = Vec::new();
        let mut v = *self;
        for i in 0..64 {
            if v & 1 == 1 {
                attributes.push(i);
            }
            v = v.rotate_right(1);
        }

        let mut s = Vec::new();
        for a in attributes {
            s.push(match a {
                0 => "0:RequiredPartition".to_string(),
                1 => "1:NoBlockIOProtocol".to_string(),
                2 => "2:LegacyBIOSBootable".to_string(),
                x if x < 48 => format!("{}:Reserved", x),
                x => match &type_guid {
                    BASIC_DATA_PARTITION => match a {
                        60 => "60:ReadOnly".to_string(),
                        61 => "61:ShadowCopy".to_string(),
                        62 => "62:Hidden".to_string(),
                        63 => "63:NoDriveLetter".to_string(),
                        x => format!("{}", x),
                    },
                    _ => format!("{}", x),
                },
            });
        }

        s.join(",")
    }
}
