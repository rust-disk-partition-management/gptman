//! CRC-32 as the GPT specification requires it: the reflected IEEE 802.3 polynomial, starting
//! from and finishing with the same constants every other implementation uses.
//!
//! The two checksums of a GPT are computed over 92 bytes of header and over the partition entry
//! array, so a byte-at-a-time implementation is far from being a bottleneck and the `crc` crate
//! is not needed to get a table-driven one.
//!
//! The table is a single `const`, so it lives in read-only data and every caller shares it.
//! Computing a checksum therefore takes no stack and no heap, which is what a `no_std` build
//! needs, and unlike a `crc` crate `Crc::<u32>` constructed inside a function it does not copy a
//! kilobyte of table onto the stack.

/// The reflected IEEE 802.3 polynomial, `0x04c11db7` with its bits reversed.
const POLYNOMIAL: u32 = 0xedb8_8320;

/// The value the checksum starts from, and the one it is exclusive-ored with at the end.
const INIT: u32 = 0xffff_ffff;

/// The checksum of every byte value, built at compile time.
const TABLE: [u32; 256] = build_table();

const fn build_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut i = 0;
    while i < table.len() {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ POLYNOMIAL
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// A CRC-32 computed over a stream of bytes, one chunk at a time.
#[derive(Debug)]
pub(crate) struct Crc32 {
    value: u32,
}

impl Crc32 {
    /// Starts a new checksum.
    pub(crate) const fn new() -> Crc32 {
        Crc32 { value: INIT }
    }

    /// Adds the bytes of a chunk to the checksum.
    pub(crate) fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            let index = ((self.value ^ u32::from(*byte)) & 0xff) as usize;
            self.value = (self.value >> 8) ^ TABLE[index];
        }
    }

    /// Returns the checksum of every byte given so far.
    pub(crate) fn finalize(self) -> u32 {
        self.value ^ INIT
    }
}

/// Returns the CRC-32 of a buffer.
pub(crate) fn checksum(bytes: &[u8]) -> u32 {
    let mut crc = Crc32::new();
    crc.update(bytes);
    crc.finalize()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn checksum_of_the_check_value() {
        // The check value of the specification: CRC-32 of "123456789" is 0xCBF43926.
        assert_eq!(checksum(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn checksum_of_an_empty_buffer() {
        assert_eq!(checksum(b""), 0);
    }

    #[test]
    fn incremental_checksum_matches_a_single_pass() {
        let bytes = [0x12; 300];

        let mut crc = Crc32::new();
        crc.update(&bytes[..1]);
        crc.update(&bytes[1..100]);
        crc.update(&bytes[100..]);

        assert_eq!(crc.finalize(), checksum(&bytes));
    }
}
