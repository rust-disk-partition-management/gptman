#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

use crc::{Crc, CRC_32_ISO_HDLC};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::{Index, IndexMut, RangeInclusive};

/// Linux specific helpers
#[cfg(all(target_os = "linux", feature = "nix"))]
pub mod linux;

const DEFAULT_ALIGN: u64 = 2048;
const MAX_ALIGN: u64 = 16384;
/// Size (in bytes) of a GPT header on disk.
const GPT_HEADER_SIZE: usize = 92;
/// Size (in bytes) of a partition entry on disk.
const GPT_PARTITION_ENTRY_SIZE: usize = 128;
/// Size (in bytes) of a partition name on disk.
const PARTITION_NAME_SIZE: usize = 72;
/// Maximum size (in bytes) of the partition entry array that is accepted on read or write.
///
/// The UEFI specification requires the array to fit between the header and the first usable LBA,
/// so the array of a disk that any system would boot from is far below this. It keeps a header
/// read from an untrusted image from requesting an unreasonable allocation.
const MAX_PARTITION_ENTRY_ARRAY_SIZE: u64 = 16 * 1024 * 1024;

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(read_array(bytes, offset))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(read_array(bytes, offset))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    bytes[offset..offset + N]
        .try_into()
        .expect("the slice holds at least N bytes")
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

/// An error that can be produced while reading, writing or managing a GPT.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// I/O errors.
    Io(io::Error),
    /// An error that occurs when the signature of the GPT isn't what would be expected ("EFI
    /// PART").
    InvalidSignature,
    /// An error that occurs when the revision of the GPT isn't what would be expected (00 00 01
    /// 00).
    InvalidRevision,
    /// An error that occurs when the header's size (in bytes) isn't what would be expected (92).
    InvalidHeaderSize,
    /// An error that occurs when the size (in bytes) of a partition entry isn't the one gptman
    /// supports (128).
    InvalidPartitionEntrySize(u32),
    /// An error that occurs when the partition entry array is larger than gptman accepts.
    ///
    /// The argument is the size (in bytes) of the array described by the header.
    InvalidPartitionEntryArraySize(u64),
    /// An error that occurs when the number of partition entries given does not match the number
    /// the header describes.
    ///
    /// The first argument is the number of entries given, the second the number of entries the
    /// header describes.
    InvalidPartitionEntryCount(u64, u64),
    /// An error that occurs when the CRC32 checksum of the header doesn't match the expected
    /// checksum for the actual header.
    InvalidChecksum(u32, u32),
    /// An error that occurs when the CRC32 checksum of the partition entries array doesn't match
    /// the expected checksum for the actual partition entries array.
    InvalidPartitionEntryArrayChecksum(u32, u32),
    /// An error that occurs when reading a GPT from a file did not succeed.
    ///
    /// The first argument is the error that occurred when trying to read the primary header.
    /// The second argument is the error that occurred when trying to read the backup header.
    ReadError(Box<Error>, Box<Error>),
    /// An error that occurs when there is not enough space left on the table to continue.
    NoSpaceLeft,
    /// An error that occurs when there are partitions with the same GUID in the same array.
    ConflictPartitionGUID,
    /// An error that occurs when a partition has an invalid boundary.
    ///
    /// The end sector must be greater or equal to the start sector of the partition.
    ///
    /// Partitions must fit within the disk and must not overlap.
    InvalidPartitionBoundaries,
    /// An error that occurs when the user provide an invalid partition number.
    ///
    /// The partition number must be between 1 and `number_of_partition_entries` (usually 128)
    /// included.
    InvalidPartitionNumber(u32),
    /// An error that occurs when the user attempts to access information for an unused partition.
    UnusedPartition,
    /// An operation that required to find a partition, was unable to find that partition.
    PartitionNotFound,
    /// An arithmetic operation overflowed.
    Overflow,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Io(_) => f.write_str("generic I/O error"),
            Error::InvalidSignature => f.write_str("invalid signature"),
            Error::InvalidRevision => f.write_str("invalid revision"),
            Error::InvalidHeaderSize => f.write_str("invalid header size"),
            Error::InvalidPartitionEntrySize(size) => {
                write!(f, "invalid partition entry size: {size}")
            }
            Error::InvalidPartitionEntryArraySize(size) => {
                write!(f, "partition entry array too large: {size} bytes")
            }
            Error::InvalidPartitionEntryCount(given, described) => {
                write!(
                    f,
                    "invalid number of partition entries ({given} != {described})"
                )
            }
            Error::InvalidChecksum(found, expected) => {
                write!(f, "corrupted CRC32 checksum ({found} != {expected})")
            }
            Error::InvalidPartitionEntryArrayChecksum(found, expected) => write!(
                f,
                "corrupted partition entry array CRC32 checksum ({found} != {expected})"
            ),
            Error::ReadError(primary, backup) => write!(
                f,
                "could not read primary header ({primary}) nor backup header ({backup})"
            ),
            Error::NoSpaceLeft => f.write_str("no space left"),
            Error::ConflictPartitionGUID => f.write_str("conflict of partition GUIDs"),
            Error::InvalidPartitionBoundaries => f.write_str(
                "invalid partition boundaries: partitions must have positive size, must not \
                overlap, and must fit within the disk",
            ),
            Error::InvalidPartitionNumber(number) => {
                write!(f, "invalid partition number: {number}")
            }
            Error::UnusedPartition => f.write_str("unused partition"),
            Error::PartitionNotFound => f.write_str("partition not found"),
            Error::Overflow => f.write_str("an arithmetic operation overflowed"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

/// The result of reading, writing or managing a GPT.
pub type Result<T> = std::result::Result<T, Error>;

/// A GUID Partition Table header as describe on
/// [Wikipedia's page](https://en.wikipedia.org/wiki/GUID_Partition_Table#Partition_table_header_(LBA_1)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GPTHeader {
    /// GPT signature (must be "EFI PART").
    pub signature: [u8; 8],
    /// GPT revision (must be 00 00 01 00).
    pub revision: [u8; 4],
    /// GPT header size (must be 92).
    pub header_size: u32,
    /// CRC32 checksum of the header.
    pub crc32_checksum: u32,
    /// Reserved bytes of the header.
    pub reserved: [u8; 4],
    /// Location (in sectors) of the primary header.
    pub primary_lba: u64,
    /// Location (in sectors) of the backup header.
    pub backup_lba: u64,
    /// Location (in sectors) of the first usable sector.
    pub first_usable_lba: u64,
    /// Location (in sectors) of the last usable sector.
    pub last_usable_lba: u64,
    /// 16 bytes representing the UUID of the GPT.
    pub disk_guid: [u8; 16],
    /// Location (in sectors) of the partition entries array.
    ///
    /// This is always `2` if the header is a primary header and not a backup header.
    pub partition_entry_lba: u64,
    /// Number of partition entries in the array.
    pub number_of_partition_entries: u32,
    /// Size (in bytes) of a partition entry.
    pub size_of_partition_entry: u32,
    /// CRC32 checksum of the partition array.
    pub partition_entry_array_crc32: u32,
}

impl GPTHeader {
    /// Make a new GPT header based on a reader. (This operation does not write anything to disk!)
    pub fn new_from<R>(reader: &mut R, sector_size: u64, disk_guid: [u8; 16]) -> Result<GPTHeader>
    where
        R: Read + Seek,
    {
        let mut gpt = GPTHeader {
            signature: [0x45, 0x46, 0x49, 0x20, 0x50, 0x41, 0x52, 0x54],
            revision: [0x00, 0x00, 0x01, 0x00],
            header_size: 92,
            crc32_checksum: 0,
            reserved: [0; 4],
            primary_lba: 1,
            backup_lba: 0,
            first_usable_lba: 0,
            last_usable_lba: 0,
            disk_guid,
            partition_entry_lba: 2,
            number_of_partition_entries: 128,
            size_of_partition_entry: 128,
            partition_entry_array_crc32: 0,
        };
        gpt.update_from(reader, sector_size)?;

        Ok(gpt)
    }

    /// Read a GPT header from a reader.
    ///
    /// The header is decoded and validated like [`GPTHeader::from_bytes`] does.
    ///
    /// # Implementation notes
    ///
    /// The field `last_usable_lba` is not updated to reflect the actual size of the disk. You must
    /// do this yourself by calling `update_from`.
    pub fn read_from<R>(reader: &mut R) -> Result<GPTHeader>
    where
        R: Read + Seek + ?Sized,
    {
        let mut bytes = [0; GPT_HEADER_SIZE];
        reader.read_exact(&mut bytes)?;
        GPTHeader::from_bytes(&bytes)
    }

    /// Decode a GPT header from its on-disk representation.
    ///
    /// The signature, the revision, the header size and the CRC32 checksum must be the ones the
    /// specification requires. The partition entry size must be 128 bytes, the only size gptman
    /// reads and writes, and the partition entry array must not exceed
    /// `MAX_PARTITION_ENTRY_ARRAY_SIZE` bytes, so that a header read from an untrusted image
    /// cannot request an unreasonable allocation.
    pub fn from_bytes(bytes: &[u8; GPT_HEADER_SIZE]) -> Result<GPTHeader> {
        let gpt = GPTHeader {
            signature: read_array(bytes, 0),
            revision: read_array(bytes, 8),
            header_size: read_u32(bytes, 12),
            crc32_checksum: read_u32(bytes, 16),
            reserved: read_array(bytes, 20),
            primary_lba: read_u64(bytes, 24),
            backup_lba: read_u64(bytes, 32),
            first_usable_lba: read_u64(bytes, 40),
            last_usable_lba: read_u64(bytes, 48),
            disk_guid: read_array(bytes, 56),
            partition_entry_lba: read_u64(bytes, 72),
            number_of_partition_entries: read_u32(bytes, 80),
            size_of_partition_entry: read_u32(bytes, 84),
            partition_entry_array_crc32: read_u32(bytes, 88),
        };

        if &gpt.signature != b"EFI PART" {
            return Err(Error::InvalidSignature);
        }

        if gpt.revision != [0x00, 0x00, 0x01, 0x00] {
            return Err(Error::InvalidRevision);
        }

        if gpt.header_size != GPT_HEADER_SIZE as u32 {
            return Err(Error::InvalidHeaderSize);
        }

        let sum = gpt.generate_crc32_checksum();
        if gpt.crc32_checksum != sum {
            return Err(Error::InvalidChecksum(gpt.crc32_checksum, sum));
        }

        gpt.check_partition_entry_array()?;

        Ok(gpt)
    }

    /// Encode the GPT header to its on-disk representation.
    pub fn to_bytes(&self) -> [u8; GPT_HEADER_SIZE] {
        let mut bytes = [0; GPT_HEADER_SIZE];
        bytes[0..8].copy_from_slice(&self.signature);
        bytes[8..12].copy_from_slice(&self.revision);
        write_u32(&mut bytes, 12, self.header_size);
        write_u32(&mut bytes, 16, self.crc32_checksum);
        bytes[20..24].copy_from_slice(&self.reserved);
        write_u64(&mut bytes, 24, self.primary_lba);
        write_u64(&mut bytes, 32, self.backup_lba);
        write_u64(&mut bytes, 40, self.first_usable_lba);
        write_u64(&mut bytes, 48, self.last_usable_lba);
        bytes[56..72].copy_from_slice(&self.disk_guid);
        write_u64(&mut bytes, 72, self.partition_entry_lba);
        write_u32(&mut bytes, 80, self.number_of_partition_entries);
        write_u32(&mut bytes, 84, self.size_of_partition_entry);
        write_u32(&mut bytes, 88, self.partition_entry_array_crc32);
        bytes
    }

    /// Check that the partition entry array of this header is one gptman can read and write.
    fn check_partition_entry_array(&self) -> Result<()> {
        if self.size_of_partition_entry != GPT_PARTITION_ENTRY_SIZE as u32 {
            return Err(Error::InvalidPartitionEntrySize(
                self.size_of_partition_entry,
            ));
        }

        let size =
            u64::from(self.number_of_partition_entries) * u64::from(self.size_of_partition_entry);
        if size > MAX_PARTITION_ENTRY_ARRAY_SIZE {
            return Err(Error::InvalidPartitionEntryArraySize(size));
        }

        Ok(())
    }

    /// Read and validate the partition entry array that belongs to this header.
    ///
    /// Seeks to the array using `partition_entry_lba` and `sector_size`, reads all entries, then
    /// verifies `partition_entry_array_crc32` against the bytes read.
    pub fn read_partitions<R>(
        &self,
        reader: &mut R,
        sector_size: u64,
    ) -> Result<Vec<GPTPartitionEntry>>
    where
        R: Read + Seek + ?Sized,
    {
        self.check_partition_entry_array()?;

        let size = self.partition_entry_array_size() as usize;
        reader.seek(SeekFrom::Start(
            self.partition_entry_lba
                .checked_mul(sector_size)
                .ok_or(Error::Overflow)?,
        ))?;
        let mut data = vec![0; size];
        reader.read_exact(&mut data)?;

        let sum = Crc::<u32>::new(&CRC_32_ISO_HDLC).checksum(&data);
        if self.partition_entry_array_crc32 != sum {
            return Err(Error::InvalidPartitionEntryArrayChecksum(
                self.partition_entry_array_crc32,
                sum,
            ));
        }

        Ok(data
            .chunks_exact(GPT_PARTITION_ENTRY_SIZE)
            .map(|x| GPTPartitionEntry::from_bytes(&read_array(x, 0)))
            .collect())
    }

    /// Write the GPT header into a writer. This operation will update the CRC32 checksums of the
    /// current struct and seek at the location `primary_lba` before trying to write to disk.
    ///
    /// # Errors
    ///
    /// The header has to be one gptman can read back: a size of 92 bytes, a partition entry size
    /// of 128 bytes, and as many partition entries as `number_of_partition_entries`.
    pub fn write_into<W>(
        &mut self,
        writer: &mut W,
        sector_size: u64,
        partitions: &[GPTPartitionEntry],
    ) -> Result<()>
    where
        W: Write + Seek + ?Sized,
    {
        if self.header_size != GPT_HEADER_SIZE as u32 {
            return Err(Error::InvalidHeaderSize);
        }
        self.check_partition_entry_array()?;
        self.update_partition_entry_array_crc32(partitions)?;
        self.update_crc32_checksum();

        writer.seek(SeekFrom::Start(
            self.primary_lba
                .checked_mul(sector_size)
                .ok_or(Error::Overflow)?,
        ))?;
        writer.write_all(&self.to_bytes())?;

        let array_offset = self
            .partition_entry_lba
            .checked_mul(sector_size)
            .ok_or(Error::Overflow)?;
        for (i, partition) in partitions.iter().enumerate() {
            writer.seek(SeekFrom::Start(
                array_offset
                    .checked_add((i * GPT_PARTITION_ENTRY_SIZE) as u64)
                    .ok_or(Error::Overflow)?,
            ))?;
            writer.write_all(&partition.to_bytes())?;
        }

        Ok(())
    }

    /// The size (in bytes) of the partition entry array this header describes.
    fn partition_entry_array_size(&self) -> u64 {
        u64::from(self.number_of_partition_entries) * u64::from(self.size_of_partition_entry)
    }

    /// Generate the CRC32 checksum of the partition header only.
    pub fn generate_crc32_checksum(&self) -> u32 {
        let mut bytes = self.to_bytes();
        write_u32(&mut bytes, 16, 0);
        Crc::<u32>::new(&CRC_32_ISO_HDLC).checksum(&bytes)
    }

    /// Update the CRC32 checksum of this header.
    pub fn update_crc32_checksum(&mut self) {
        self.crc32_checksum = self.generate_crc32_checksum();
    }

    /// Generate the CRC32 checksum of the partition entry array.
    ///
    /// # Errors
    ///
    /// The array has to be the one this header describes: as many entries as
    /// `number_of_partition_entries`.
    pub fn generate_partition_entry_array_crc32(
        &self,
        partitions: &[GPTPartitionEntry],
    ) -> Result<u32> {
        self.check_partition_entry_array()?;
        self.check_partitions_len(partitions)?;

        let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
        let mut digest = crc.digest();
        for x in partitions {
            digest.update(&x.to_bytes());
        }

        Ok(digest.finalize())
    }

    /// Update the CRC32 checksum of the partition entry array.
    ///
    /// # Errors
    ///
    /// The array has to be the one this header describes: as many entries as
    /// `number_of_partition_entries`.
    pub fn update_partition_entry_array_crc32(
        &mut self,
        partitions: &[GPTPartitionEntry],
    ) -> Result<()> {
        self.partition_entry_array_crc32 = self.generate_partition_entry_array_crc32(partitions)?;
        Ok(())
    }

    /// Check that `partitions` holds exactly the entries this header describes.
    fn check_partitions_len(&self, partitions: &[GPTPartitionEntry]) -> Result<()> {
        if partitions.len() != self.number_of_partition_entries as usize {
            return Err(Error::InvalidPartitionEntryCount(
                partitions.len() as u64,
                u64::from(self.number_of_partition_entries),
            ));
        }

        Ok(())
    }

    /// Updates the header to match the specifications of the seeker given in argument.
    /// `first_usable_lba`, `last_usable_lba`, `primary_lba`, `backup_lba`, `partition_entry_lba`
    /// will be updated after this operation.
    pub fn update_from<S>(&mut self, seeker: &mut S, sector_size: u64) -> Result<()>
    where
        S: Seek + ?Sized,
    {
        let partition_array_size = (u64::from(self.number_of_partition_entries)
            * u64::from(self.size_of_partition_entry)
            - 1)
            / sector_size
            + 1;
        let len = seeker.seek(SeekFrom::End(0))? / sector_size;
        if self.primary_lba == 1 {
            self.backup_lba = len - 1;
        } else {
            self.primary_lba = len - 1;
        }
        self.last_usable_lba = len - partition_array_size - 1 - 1;
        self.first_usable_lba = 2 + partition_array_size;
        // NOTE: the partition_entry_lba is either 2 either something near the end of the disk.
        //       If it is something near the end of the disk, it means the self object is a backup
        //       GPT header (which is located at the end of the disk) and its partition_entry_lba
        //       must be updated accordingly
        if self.partition_entry_lba != 2 {
            self.partition_entry_lba = self.last_usable_lba + 1;
        }

        Ok(())
    }

    /// Returns `true` if the `GPTHeader` is a primary copy (the header is located at the beginning
    /// of the disk).
    pub fn is_primary(&self) -> bool {
        self.primary_lba == 1
    }

    /// Returns `true` if the `GPTHeader` is a backup copy (the header is located at the end of the
    /// disk).
    ///
    /// Note that when the header is a backup copy, the `primary_lba` is the LBA of the backup copy
    /// and the `backup_lba` is the LBA of the primary copy.
    pub fn is_backup(&self) -> bool {
        !self.is_primary()
    }
}

/// A wrapper type for `String` that represents a partition's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionName {
    string: String,
    raw_buf: [u16; 36],
}

impl PartitionName {
    /// Extracts a string slice containing the entire `PartitionName`.
    pub fn as_str(&self) -> &str {
        &self.string
    }

    /// Decode a partition name from its on-disk representation: 36 UTF-16LE code units.
    ///
    /// The string stops at the first null code unit; anything after it is kept so that a name
    /// encoded again, or used for a checksum, is byte-for-byte identical to what was read.
    pub fn from_bytes(bytes: &[u8; PARTITION_NAME_SIZE]) -> PartitionName {
        let mut raw_buf = [0; 36];
        for (i, x) in raw_buf.iter_mut().enumerate() {
            *x = u16::from_le_bytes(read_array(bytes, 2 * i));
        }

        let zero_pos = raw_buf
            .iter()
            .position(|x| *x == 0)
            .unwrap_or(raw_buf.len());

        PartitionName {
            string: String::from_utf16_lossy(&raw_buf[..zero_pos]),
            raw_buf,
        }
    }

    /// Encode the partition name to its on-disk representation.
    ///
    /// The bytes read from disk are used rather than the string, so that garbage left after the
    /// null terminator of a name read from disk is written back unchanged.
    pub fn to_bytes(&self) -> [u8; PARTITION_NAME_SIZE] {
        let mut bytes = [0; PARTITION_NAME_SIZE];
        for (i, x) in self.raw_buf.iter().enumerate() {
            bytes[2 * i..2 * i + 2].copy_from_slice(&x.to_le_bytes());
        }
        bytes
    }
}

impl std::fmt::Display for PartitionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.string)
    }
}

impl From<&str> for PartitionName {
    fn from(value: &str) -> PartitionName {
        let utf16_converted: Vec<_> = value
            .encode_utf16()
            .chain([0].into_iter().cycle())
            .take(36)
            .collect();
        let mut raw_buf = [0; 36];
        raw_buf.copy_from_slice(&utf16_converted);
        PartitionName {
            string: value.to_string(),
            raw_buf,
        }
    }
}

/// A GPT partition's entry in the partition array.
///
/// # Examples
///
/// Basic usage:
/// ```
/// let ss = 512;
/// let data = vec![0; 100 * ss as usize];
/// let mut cur = std::io::Cursor::new(data);
/// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
///     .expect("could not create partition table");
///
/// // NOTE: partition entries starts at 1
/// gpt[1] = gptman::GPTPartitionEntry {
///     partition_type_guid: [0xff; 16],
///     unique_partition_guid: [0xff; 16],
///     starting_lba: gpt.header.first_usable_lba,
///     ending_lba: gpt.header.last_usable_lba,
///     attribute_bits: 0,
///     partition_name: "A Robot Named Fight!".into(),
/// };
///
/// assert_eq!(gpt[1].partition_name.as_str(), "A Robot Named Fight!");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GPTPartitionEntry {
    /// 16 bytes representing the UUID of the partition's type.
    pub partition_type_guid: [u8; 16],
    /// 16 bytes representing the UUID of the partition.
    pub unique_partition_guid: [u8; 16],
    /// The position (in sectors) of the first sector (used) of the partition.
    pub starting_lba: u64,
    /// The position (in sectors) of the last sector (used) of the partition.
    pub ending_lba: u64,
    /// The attribute bits.
    ///
    /// See [Wikipedia's page](https://en.wikipedia.org/wiki/GUID_Partition_Table#Partition_entries_(LBA_2%E2%80%9333))
    /// for more information.
    pub attribute_bits: u64,
    /// The partition name.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let name: gptman::PartitionName = "A Robot Named Fight!".into();
    ///
    /// assert_eq!(name.as_str(), "A Robot Named Fight!");
    /// ```
    pub partition_name: PartitionName,
}

impl GPTPartitionEntry {
    /// Creates an empty partition entry
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not create partition table");
    ///
    /// gpt[1] = gptman::GPTPartitionEntry::empty();
    ///
    /// // NOTE: an empty partition entry is considered as not allocated
    /// assert!(gpt[1].is_unused());
    /// ```
    pub fn empty() -> GPTPartitionEntry {
        GPTPartitionEntry {
            partition_type_guid: [0; 16],
            unique_partition_guid: [0; 16],
            starting_lba: 0,
            ending_lba: 0,
            attribute_bits: 0,
            partition_name: "".into(),
        }
    }

    /// Read a partition entry from the reader at the current position.
    ///
    /// The entry is decoded like [`GPTPartitionEntry::from_bytes`] does.
    pub fn read_from<R>(reader: &mut R) -> Result<GPTPartitionEntry>
    where
        R: Read + ?Sized,
    {
        let mut bytes = [0; GPT_PARTITION_ENTRY_SIZE];
        reader.read_exact(&mut bytes)?;
        Ok(GPTPartitionEntry::from_bytes(&bytes))
    }

    /// Decode a partition entry from its on-disk representation.
    pub fn from_bytes(bytes: &[u8; GPT_PARTITION_ENTRY_SIZE]) -> GPTPartitionEntry {
        GPTPartitionEntry {
            partition_type_guid: read_array(bytes, 0),
            unique_partition_guid: read_array(bytes, 16),
            starting_lba: read_u64(bytes, 32),
            ending_lba: read_u64(bytes, 40),
            attribute_bits: read_u64(bytes, 48),
            partition_name: PartitionName::from_bytes(&read_array(bytes, 56)),
        }
    }

    /// Encode the partition entry to its on-disk representation.
    pub fn to_bytes(&self) -> [u8; GPT_PARTITION_ENTRY_SIZE] {
        let mut bytes = [0; GPT_PARTITION_ENTRY_SIZE];
        bytes[0..16].copy_from_slice(&self.partition_type_guid);
        bytes[16..32].copy_from_slice(&self.unique_partition_guid);
        write_u64(&mut bytes, 32, self.starting_lba);
        write_u64(&mut bytes, 40, self.ending_lba);
        write_u64(&mut bytes, 48, self.attribute_bits);
        bytes[56..128].copy_from_slice(&self.partition_name.to_bytes());
        bytes
    }

    /// Returns `true` if the partition entry is not used (type GUID == `[0; 16]`)
    pub fn is_unused(&self) -> bool {
        self.partition_type_guid == [0; 16]
    }

    /// Returns `true` if the partition entry is used (type GUID != `[0; 16]`)
    pub fn is_used(&self) -> bool {
        !self.is_unused()
    }

    /// Returns the number of sectors in the partition. A partition entry must always be 1 sector
    /// long at minimum.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `ending_lba` is lesser than the `starting_lba`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not create partition table");
    ///
    /// gpt[1] = gptman::GPTPartitionEntry {
    ///     partition_type_guid: [0xff; 16],
    ///     unique_partition_guid: [0xff; 16],
    ///     starting_lba: gpt.header.first_usable_lba,
    ///     ending_lba: gpt.header.last_usable_lba,
    ///     attribute_bits: 0,
    ///     partition_name: "A Robot Named Fight!".into(),
    /// };
    ///
    /// assert_eq!(
    ///     gpt[1].size().ok(),
    ///     Some(gpt.header.last_usable_lba + 1 - gpt.header.first_usable_lba)
    /// );
    /// ```
    pub fn size(&self) -> Result<u64> {
        if self.ending_lba < self.starting_lba {
            return Err(Error::InvalidPartitionBoundaries);
        }

        Ok(self.ending_lba - self.starting_lba + 1)
    }

    /// Get the range of sectors covered by a partition.
    ///
    /// # Errors
    ///
    /// This function will return an error if the partition is unused or if
    /// `ending_lba` is lesser than the `starting_lba`.
    ///
    /// # Examples
    ///
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not create partition table");
    /// gpt[1] = gptman::GPTPartitionEntry {
    ///     partition_type_guid: [0xff; 16],
    ///     unique_partition_guid: [0xff; 16],
    ///     starting_lba: 2048,
    ///     ending_lba: 4096,
    ///     attribute_bits: 0,
    ///     partition_name: "A Robot Named Fight!".into(),
    /// };
    ///
    /// assert_eq!(gpt[1].range().unwrap(), 2048..=4096);
    /// ```
    pub fn range(&self) -> Result<RangeInclusive<u64>> {
        if self.is_unused() {
            return Err(Error::UnusedPartition);
        }
        if self.ending_lba < self.starting_lba {
            return Err(Error::InvalidPartitionBoundaries);
        }

        Ok(self.starting_lba..=self.ending_lba)
    }
}

/// A type representing a GUID partition table including its partitions, the sector size of the
/// disk and the alignment of the partitions to the sectors.
///
/// # Examples
///
/// Read an existing GPT on a reader and list its partitions:
/// ```
/// let mut f = std::fs::File::open("tests/fixtures/disk1.img")
///     .expect("could not open disk");
/// let gpt = gptman::GPT::find_from(&mut f)
///     .expect("could not find GPT");
///
/// println!("Disk GUID: {:?}", gpt.header.disk_guid);
///
/// for (i, p) in gpt.iter() {
///     if p.is_used() {
///         println!("Partition #{}: type = {:?}, size = {} bytes, starting lba = {}",
///             i,
///             p.partition_type_guid,
///             p.size().unwrap() * gpt.sector_size,
///             p.starting_lba);
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GPT {
    /// Sector size of the disk.
    ///
    /// You should not change this, otherwise the starting locations of your partitions will be
    /// different in bytes.
    pub sector_size: u64,
    /// GPT partition header (disk GUID, first/last usable LBA, etc...)
    pub header: GPTHeader,
    partitions: Vec<GPTPartitionEntry>,
    /// Partitions alignment (in sectors)
    ///
    /// This field change the behavior of the methods `get_maximum_partition_size()`,
    /// `find_free_sectors()`, `find_first_place()`, `find_last_place()` and `find_optimal_place()`
    /// so they return only values aligned to the alignment.
    ///
    /// # Panics
    ///
    /// The value must be greater than 0, otherwise you will encounter divisions by zero.
    pub align: u64,
}

impl GPT {
    /// Make a new GPT based on a reader. (This operation does not write anything to disk!)
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not make a partition table");
    /// ```
    pub fn new_from<R>(reader: &mut R, sector_size: u64, disk_guid: [u8; 16]) -> Result<GPT>
    where
        R: Read + Seek,
    {
        let header = GPTHeader::new_from(reader, sector_size, disk_guid)?;
        let mut partitions = Vec::with_capacity(header.number_of_partition_entries as usize);
        for _ in 0..header.number_of_partition_entries {
            partitions.push(GPTPartitionEntry::empty());
        }

        Ok(GPT {
            sector_size,
            header,
            partitions,
            align: DEFAULT_ALIGN,
        })
    }

    /// Read the GPT on a reader. This function will try to read the backup header if the primary
    /// header could not be read.
    ///
    /// # Implementation notes
    ///
    /// The field `last_usable_lba` on the header is not updated to reflect the actual size of the
    /// disk. You must do this yourself by calling `update_from`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let mut f = std::fs::File::open("tests/fixtures/disk1.img")
    ///     .expect("could not open disk");
    /// let gpt = gptman::GPT::read_from(&mut f, 512)
    ///     .expect("could not read the partition table");
    /// ```
    pub fn read_from<R>(mut reader: &mut R, sector_size: u64) -> Result<GPT>
    where
        R: Read + Seek + ?Sized,
    {
        use self::Error::*;

        let primary: Result<(GPTHeader, Vec<GPTPartitionEntry>)> = (|| {
            reader.seek(SeekFrom::Start(sector_size))?;
            let header = GPTHeader::read_from(&mut reader)?;
            let partitions = header.read_partitions(&mut reader, sector_size)?;
            Ok((header, partitions))
        })();

        let (header, partitions) = primary.or_else(|primary_err| {
            let len = reader.seek(SeekFrom::End(0))?;
            if len < sector_size {
                return Err(InvalidSignature);
            }

            reader.seek(SeekFrom::Start((len / sector_size - 1) * sector_size))?;

            GPTHeader::read_from(&mut reader)
                .and_then(|header| {
                    let partitions = header.read_partitions(&mut reader, sector_size)?;
                    Ok((header, partitions))
                })
                .map_err(|backup_err| match (primary_err, backup_err) {
                    (InvalidSignature, InvalidSignature) => InvalidSignature,
                    (x, y) => ReadError(Box::new(x), Box::new(y)),
                })
        })?;

        let align = GPT::find_alignment(&header, &partitions);

        Ok(GPT {
            sector_size,
            header,
            partitions,
            align,
        })
    }

    /// Find the GPT on a reader. This function will try to read the GPT on a disk using a sector
    /// size of 512 but if it fails it will automatically try to read the GPT using a sector size
    /// of 4096.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let mut f_512 = std::fs::File::open("tests/fixtures/disk1.img")
    ///     .expect("could not open disk");
    /// let gpt_512 = gptman::GPT::find_from(&mut f_512)
    ///     .expect("could not read the partition table");
    ///
    /// let mut f_4096 = std::fs::File::open("tests/fixtures/disk2.img")
    ///     .expect("could not open disk");
    /// let gpt_4096 = gptman::GPT::find_from(&mut f_4096)
    ///     .expect("could not read the partition table");
    /// ```
    pub fn find_from<R>(mut reader: &mut R) -> Result<GPT>
    where
        R: Read + Seek + ?Sized,
    {
        use self::Error::*;

        Self::read_from(&mut reader, 512).or_else(|err_at_512| match err_at_512 {
            InvalidSignature => Self::read_from(&mut reader, 4096),
            err => Err(err),
        })
    }

    fn find_alignment(header: &GPTHeader, partitions: &[GPTPartitionEntry]) -> u64 {
        let lbas = partitions
            .iter()
            .filter(|x| x.is_used())
            .map(|x| x.starting_lba)
            .collect::<Vec<_>>();

        if lbas.is_empty() {
            return DEFAULT_ALIGN;
        }

        if lbas.len() == 1 && lbas[0] == header.first_usable_lba {
            return 1;
        }

        (1..=MAX_ALIGN.min(*lbas.iter().max().unwrap_or(&1)))
            .filter(|div| lbas.iter().all(|x| x % div == 0))
            .max()
            .unwrap()
    }

    fn check_partition_guids(&self) -> Result<()> {
        let guids: Vec<_> = self
            .partitions
            .iter()
            .filter(|x| x.is_used())
            .map(|x| x.unique_partition_guid)
            .collect();
        if guids.len() != guids.iter().collect::<HashSet<_>>().len() {
            return Err(Error::ConflictPartitionGUID);
        }

        Ok(())
    }

    fn check_partition_boundaries(&self) -> Result<()> {
        if self
            .partitions
            .iter()
            .any(|x| x.ending_lba < x.starting_lba)
        {
            return Err(Error::InvalidPartitionBoundaries);
        }

        let mut partitions: Vec<&GPTPartitionEntry> =
            self.partitions.iter().filter(|x| x.is_used()).collect();
        partitions.sort_unstable_by_key(|x| x.starting_lba);
        let first_available =
            partitions
                .iter()
                .try_fold(self.header.first_usable_lba, |first_available, x| {
                    if x.starting_lba >= first_available {
                        Ok(x.ending_lba + 1)
                    } else {
                        Err(Error::InvalidPartitionBoundaries)
                    }
                })?;
        if first_available > self.header.last_usable_lba + 1 {
            return Err(Error::InvalidPartitionBoundaries);
        }

        Ok(())
    }

    /// Write the GPT to a writer. This function will seek automatically in the writer to write the
    /// primary header and the backup header at their proper location.
    ///
    /// Returns the backup `GPTHeader` that has been wrote in case of success (or the primary
    /// `GPTHeader` if `self` was using a backup header).
    ///
    /// Note that the checksums are re-calculated, thus updating the header.
    ///
    /// # Errors
    ///
    /// The partitions will be checked for consistency before being wrote to disk:
    ///
    /// * the partition GUIDs must be unique,
    /// * the partitions must have positive size,
    /// * the partitions must not overlap,
    /// * the partitions must fit within the disk.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not make a partition table");
    ///
    /// // actually write:
    /// gpt.write_into(&mut cur)
    ///     .expect("could not write GPT to disk");
    /// ```
    pub fn write_into<W>(&mut self, mut writer: &mut W) -> Result<GPTHeader>
    where
        W: Write + Seek + ?Sized,
    {
        self.check_partition_guids()?;
        self.check_partition_boundaries()?;

        let mut backup = self.header.clone();
        backup.primary_lba = self.header.backup_lba;
        backup.backup_lba = self.header.primary_lba;
        backup.partition_entry_lba = if self.header.partition_entry_lba == 2 {
            self.header.last_usable_lba + 1
        } else {
            2
        };

        self.header
            .write_into(&mut writer, self.sector_size, &self.partitions)?;
        backup.write_into(&mut writer, self.sector_size, &self.partitions)?;

        Ok(backup)
    }

    /// Finds the partition where the given sector resides.
    pub fn find_at_sector(&self, sector: u64) -> Option<u32> {
        fn between(partition: &GPTPartitionEntry, sector: u64) -> bool {
            sector >= partition.starting_lba && sector <= partition.ending_lba
        }

        self.iter()
            .find(|(_, partition)| partition.is_used() && between(partition, sector))
            .map(|(id, _)| id)
    }

    /// Find free spots in the partition table.
    ///
    /// This function will return a vector of tuple with on the left: the starting LBA of the free
    /// spot; and on the right: the size (in sectors) of the free spot.
    ///
    /// This function will automatically align with the alignment defined in the `GPT`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not create partition table");
    ///
    /// gpt[1] = gptman::GPTPartitionEntry {
    ///     partition_type_guid: [0xff; 16],
    ///     unique_partition_guid: [0xff; 16],
    ///     starting_lba: gpt.header.first_usable_lba + 5,
    ///     ending_lba: gpt.header.last_usable_lba - 5,
    ///     attribute_bits: 0,
    ///     partition_name: "A Robot Named Fight!".into(),
    /// };
    ///
    /// // NOTE: align to the sectors, so we can use every last one of them
    /// // NOTE: this is only for the demonstration purpose, this is not recommended
    /// gpt.align = 1;
    ///
    /// assert_eq!(
    ///     gpt.find_free_sectors(),
    ///     vec![(gpt.header.first_usable_lba, 5), (gpt.header.last_usable_lba - 4, 5)]
    /// );
    /// ```
    pub fn find_free_sectors(&self) -> Vec<(u64, u64)> {
        assert!(self.align > 0, "align must be greater than 0");
        let mut positions = vec![self.header.first_usable_lba - 1];
        for partition in self.partitions.iter().filter(|x| x.is_used()) {
            positions.push(partition.starting_lba);
            positions.push(partition.ending_lba);
        }
        positions.push(self.header.last_usable_lba + 1);
        positions.sort_unstable();

        positions
            .chunks(2)
            .map(|x| (x[0] + 1, x[1] - x[0] - 1))
            .filter(|(_, l)| *l > 0)
            .map(|(i, l)| (i, l, ((i - 1) / self.align + 1) * self.align - i))
            .map(|(i, l, s)| (i + s, l.saturating_sub(s)))
            .filter(|(_, l)| *l > 0)
            .collect()
    }

    /// Find the first place (most on the left) where you could start a new partition of the size
    /// given in parameter.
    /// This function will automatically align with the alignment defined in the `GPT`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not create partition table");
    ///
    /// gpt[1] = gptman::GPTPartitionEntry {
    ///     partition_type_guid: [0xff; 16],
    ///     unique_partition_guid: [0xff; 16],
    ///     starting_lba: gpt.header.first_usable_lba + 5,
    ///     ending_lba: gpt.header.last_usable_lba - 5,
    ///     attribute_bits: 0,
    ///     partition_name: "A Robot Named Fight!".into(),
    /// };
    ///
    /// // NOTE: align to the sectors, so we can use every last one of them
    /// // NOTE: this is only for the demonstration purpose, this is not recommended
    /// gpt.align = 1;
    ///
    /// assert_eq!(gpt.find_first_place(5), Some(gpt.header.first_usable_lba));
    /// ```
    pub fn find_first_place(&self, size: u64) -> Option<u64> {
        self.find_free_sectors()
            .iter()
            .find(|(_, l)| *l >= size)
            .map(|(i, _)| *i)
    }

    /// Find the last place (most on the right) where you could start a new partition of the size
    /// given in parameter.
    ///
    /// This function will automatically align with the alignment defined in the `GPT`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not create partition table");
    ///
    /// gpt[1] = gptman::GPTPartitionEntry {
    ///     partition_type_guid: [0xff; 16],
    ///     unique_partition_guid: [0xff; 16],
    ///     starting_lba: gpt.header.first_usable_lba + 5,
    ///     ending_lba: gpt.header.last_usable_lba - 5,
    ///     attribute_bits: 0,
    ///     partition_name: "A Robot Named Fight!".into(),
    /// };
    ///
    /// // NOTE: align to the sectors, so we can use every last one of them
    /// // NOTE: this is only for the demonstration purpose, this is not recommended
    /// gpt.align = 1;
    ///
    /// assert_eq!(gpt.find_last_place(5), Some(gpt.header.last_usable_lba - 4));
    /// ```
    pub fn find_last_place(&self, size: u64) -> Option<u64> {
        self.find_free_sectors()
            .iter()
            .rfind(|(_, l)| *l >= size)
            .map(|(i, l)| (i + l - size) / self.align * self.align)
    }

    /// Find the most optimal place (in the smallest free space) where you could start a new
    /// partition of the size given in parameter.
    /// This function will automatically align with the alignment defined in the `GPT`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not create partition table");
    ///
    /// gpt[1] = gptman::GPTPartitionEntry {
    ///     partition_type_guid: [0xff; 16],
    ///     unique_partition_guid: [0xff; 16],
    ///     starting_lba: gpt.header.first_usable_lba + 10,
    ///     ending_lba: gpt.header.last_usable_lba - 5,
    ///     attribute_bits: 0,
    ///     partition_name: "A Robot Named Fight!".into(),
    /// };
    ///
    /// // NOTE: align to the sectors, so we can use every last one of them
    /// // NOTE: this is only for the demonstration purpose, this is not recommended
    /// gpt.align = 1;
    ///
    /// // NOTE: the space as the end is more optimal because it will allow you to still be able to
    /// //       insert a bigger partition later
    /// assert_eq!(gpt.find_optimal_place(5), Some(gpt.header.last_usable_lba - 4));
    /// ```
    pub fn find_optimal_place(&self, size: u64) -> Option<u64> {
        let mut slots = self
            .find_free_sectors()
            .into_iter()
            .filter(|(_, l)| *l >= size)
            .collect::<Vec<_>>();
        slots.sort_by(|(_, l1), (_, l2)| l1.cmp(l2));
        slots.first().map(|&(i, _)| i)
    }

    /// Get the maximum size (in sectors) of a partition you could create in the GPT.
    /// This function will automatically align with the alignment defined in the `GPT`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not create partition table");
    ///
    /// // NOTE: align to the sectors, so we can use every last one of them
    /// // NOTE: this is only for the demonstration purpose, this is not recommended
    /// gpt.align = 1;
    ///
    /// assert_eq!(
    ///     gpt.get_maximum_partition_size().unwrap_or(0),
    ///     gpt.header.last_usable_lba + 1 - gpt.header.first_usable_lba
    /// );
    /// ```
    pub fn get_maximum_partition_size(&self) -> Result<u64> {
        self.find_free_sectors()
            .into_iter()
            .map(|(_, l)| l / self.align * self.align)
            .max()
            .ok_or(Error::NoSpaceLeft)
    }

    /// Get the range of bytes covered by a partition.
    ///
    /// # Errors
    ///
    /// This function will return an error if the partition number is invalid, or if
    /// the partition is unused, or if the partition's `ending_lba` is less than its
    /// `starting_lba`.
    ///
    /// # Examples
    ///
    /// ```
    /// let ss = 512;
    /// let data = vec![0; 100 * ss as usize];
    /// let mut cur = std::io::Cursor::new(data);
    /// let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    ///     .expect("could not create partition table");
    /// gpt[1] = gptman::GPTPartitionEntry {
    ///     partition_type_guid: [0xff; 16],
    ///     unique_partition_guid: [0xff; 16],
    ///     starting_lba: 2048,
    ///     ending_lba: 2048,
    ///     attribute_bits: 0,
    ///     partition_name: "A Robot Named Fight!".into(),
    /// };
    ///
    /// assert_eq!(gpt.get_partition_byte_range(1).unwrap(), 1048576..=1049087);
    /// ```
    pub fn get_partition_byte_range(&self, partition_number: u32) -> Result<RangeInclusive<u64>> {
        if partition_number == 0 || partition_number > self.header.number_of_partition_entries {
            return Err(Error::InvalidPartitionNumber(partition_number));
        }
        let partition = &self[partition_number];

        let sector_range = partition.range()?;

        let start_byte = sector_range
            .start()
            .checked_mul(self.sector_size)
            .ok_or(Error::Overflow)?;
        // Sector ranges are inclusive, so to get the position of the end byte we need
        // to add the size of another sector, less one byte because the byte range
        // returned from this function is also inclusive.
        let end_byte = sector_range
            .end()
            .checked_mul(self.sector_size)
            .and_then(|v| v.checked_add(self.sector_size - 1))
            .ok_or(Error::Overflow)?;
        Ok(start_byte..=end_byte)
    }

    /// Sort the partition entries in the array by the starting LBA.
    pub fn sort(&mut self) {
        self.partitions
            .sort_by(|a, b| match (a.is_used(), b.is_used()) {
                (true, true) => a.starting_lba.cmp(&b.starting_lba),
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                (false, false) => Ordering::Equal,
            });
    }

    /// Remove a partition entry in the array.
    ///
    /// This is the equivalent of:
    /// `gpt[i] = gptman::GPTPartitionEntry::empty();`
    ///
    /// # Errors
    ///
    /// This function will return an error if `i` is lesser or equal to `0` or greater than the
    /// number of partition entries (which can be obtained in the header).
    pub fn remove(&mut self, i: u32) -> Result<()> {
        if i == 0 || i > self.header.number_of_partition_entries {
            return Err(Error::InvalidPartitionNumber(i));
        }

        self.partitions[i as usize - 1] = GPTPartitionEntry::empty();

        Ok(())
    }

    /// Remove a partition entry in the array that resides at a given sector.
    ///
    /// # Errors
    ///
    /// It is an error to provide a sector which does not belong to a partition.
    pub fn remove_at_sector(&mut self, sector: u64) -> Result<()> {
        self.remove(
            self.find_at_sector(sector)
                .ok_or(Error::PartitionNotFound)?,
        )
    }

    /// Get an iterator over the partition entries and their index. The index always starts at 1.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &GPTPartitionEntry)> {
        self.partitions
            .iter()
            .enumerate()
            .map(|(i, x)| (i as u32 + 1, x))
    }

    /// Get a mutable iterator over the partition entries and their index. The index always starts
    /// at 1.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u32, &mut GPTPartitionEntry)> {
        self.partitions
            .iter_mut()
            .enumerate()
            .map(|(i, x)| (i as u32 + 1, x))
    }

    /// This function writes a protective MBR in the first sector of the disk
    /// starting at byte 446 and ending at byte 511. Any existing data will be overwritten.
    ///
    /// See also: [`Self::write_bootable_protective_mbr_into`].
    pub fn write_protective_mbr_into<W>(mut writer: &mut W, sector_size: u64) -> Result<()>
    where
        W: Write + Seek + ?Sized,
    {
        Self::write_protective_mbr_into_impl(&mut writer, sector_size, false)
    }

    /// This function writes a protective MBR in the first sector of the disk
    /// starting at byte 446 and ending at byte 511. Any existing data will be overwritten.
    /// This function differs from [`Self::write_protective_mbr_into`] in that the partition in the
    /// MBR partition table is marked as bootable. Some legacy BIOS systems do not consider a disk
    /// to be bootable if there isn't an MBR partition marked as bootable in the MBR partition
    /// table.
    ///
    /// <div class="warning">Some systems will not consider a disk to be bootable in UEFI mode
    /// if the pMBR is marked as bootable, so this should only be used if booting on legacy BIOS
    /// systems is a requirement.</div>
    pub fn write_bootable_protective_mbr_into<W>(mut writer: &mut W, sector_size: u64) -> Result<()>
    where
        W: Write + Seek + ?Sized,
    {
        Self::write_protective_mbr_into_impl(&mut writer, sector_size, true)
    }

    fn write_protective_mbr_into_impl<W>(
        writer: &mut W,
        sector_size: u64,
        bootable: bool,
    ) -> Result<()>
    where
        W: Write + Seek + ?Sized,
    {
        let size = writer.seek(SeekFrom::End(0))? / sector_size - 1;
        writer.seek(SeekFrom::Start(446))?;
        // partition 1
        if bootable {
            writer.write_all(&[0x80])?;
        } else {
            writer.write_all(&[0x00])?;
        }
        writer.write_all(&[
            0x00, 0x02, 0x00, // CHS address of first absolute sector
            0xee, // partition type
            0xff, 0xff, 0xff, // CHS address of last absolute sector
            0x01, 0x00, 0x00, 0x00, // LBA of first absolute sector
        ])?;
        // number of sectors in partition 1
        let n_sectors = if size > u64::from(u32::MAX) {
            u32::MAX
        } else {
            size as u32
        };
        writer.write_all(&n_sectors.to_le_bytes())?;
        writer.write_all(&[0; 16])?; // partition 2
        writer.write_all(&[0; 16])?; // partition 3
        writer.write_all(&[0; 16])?; // partition 4
        writer.write_all(&[0x55, 0xaa])?; // signature

        Ok(())
    }

    /// Returns `true` if the `GPTHeader` is a primary copy (the header is located at the beginning
    /// of the disk).
    pub fn is_primary(&self) -> bool {
        self.header.is_primary()
    }

    /// Returns `true` if the `GPTHeader` is a backup copy (the header is located at the end of the
    /// disk).
    ///
    /// Note that when the header is a backup copy, the `primary_lba` is the LBA of the backup copy
    /// and the `backup_lba` is the LBA of the primary copy.
    pub fn is_backup(&self) -> bool {
        self.header.is_backup()
    }
}

impl Index<u32> for GPT {
    type Output = GPTPartitionEntry;

    fn index(&self, i: u32) -> &GPTPartitionEntry {
        assert!(i != 0, "invalid partition index: 0");
        &self.partitions[i as usize - 1]
    }
}

impl IndexMut<u32> for GPT {
    fn index_mut(&mut self, i: u32) -> &mut GPTPartitionEntry {
        assert!(i != 0, "invalid partition index: 0");
        &mut self.partitions[i as usize - 1]
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::disallowed_names)]

    use super::*;
    use std::fs;

    const DISK1: &str = "tests/fixtures/disk1.img";
    const DISK2: &str = "tests/fixtures/disk2.img";
    const DISK3: &str = "tests/fixtures/disk3.img";
    const DISK4: &str = "tests/fixtures/disk4.img";
    const DISK5: &str = "tests/fixtures/disk5.img";

    #[test]
    fn read_header_and_partition_entries() {
        fn test(path: &str, ss: u64) {
            let mut f = fs::File::open(path).unwrap();

            f.seek(SeekFrom::Start(ss)).unwrap();
            let mut gpt = GPTHeader::read_from(&mut f).unwrap();

            f.seek(SeekFrom::Start(gpt.backup_lba * ss)).unwrap();
            assert!(GPTHeader::read_from(&mut f).is_ok());

            f.seek(SeekFrom::Start(gpt.partition_entry_lba * ss))
                .unwrap();
            let foo = GPTPartitionEntry::read_from(&mut f).unwrap();
            assert!(!foo.is_unused());

            f.seek(SeekFrom::Start(
                gpt.partition_entry_lba * ss + u64::from(gpt.size_of_partition_entry),
            ))
            .unwrap();
            let bar = GPTPartitionEntry::read_from(&mut f).unwrap();
            assert!(!bar.is_unused());

            let mut unused = 0;
            let mut used = 0;
            let mut partitions = Vec::new();
            for i in 0..gpt.number_of_partition_entries {
                f.seek(SeekFrom::Start(
                    gpt.partition_entry_lba * ss
                        + u64::from(i) * u64::from(gpt.size_of_partition_entry),
                ))
                .unwrap();
                let partition = GPTPartitionEntry::read_from(&mut f).unwrap();

                if partition.is_unused() {
                    unused += 1;
                } else {
                    used += 1;
                }

                // NOTE: testing that encoding the PartitionName (and the whole struct) works
                let data1 = partition.to_bytes();
                f.seek(SeekFrom::Start(
                    gpt.partition_entry_lba * ss
                        + u64::from(i) * u64::from(gpt.size_of_partition_entry),
                ))
                .unwrap();
                let mut data2 = vec![0; data1.len()];
                f.read_exact(&mut data2).unwrap();
                assert_eq!(data1, data2[..]);

                partitions.push(partition);
            }
            assert_eq!(unused, 126);
            assert_eq!(used, 2);

            let sum = gpt.crc32_checksum;
            gpt.update_crc32_checksum();
            assert_eq!(gpt.crc32_checksum, sum);
            assert_eq!(gpt.generate_crc32_checksum(), sum);
            assert_ne!(gpt.crc32_checksum, 0);

            let sum = gpt.partition_entry_array_crc32;
            gpt.update_partition_entry_array_crc32(&partitions).unwrap();
            assert_eq!(gpt.partition_entry_array_crc32, sum);
            assert_eq!(
                gpt.generate_partition_entry_array_crc32(&partitions)
                    .unwrap(),
                sum
            );
            assert_ne!(gpt.partition_entry_array_crc32, 0);
        }

        test(DISK1, 512);
        test(DISK2, 4096);
    }

    #[test]
    fn read_and_find_from_primary() {
        assert!(GPT::read_from(&mut fs::File::open(DISK1).unwrap(), 512).is_ok());
        assert!(GPT::read_from(&mut fs::File::open(DISK1).unwrap(), 4096).is_err());
        assert!(GPT::read_from(&mut fs::File::open(DISK2).unwrap(), 512).is_err());
        assert!(GPT::read_from(&mut fs::File::open(DISK2).unwrap(), 4096).is_ok());
        assert!(GPT::find_from(&mut fs::File::open(DISK1).unwrap()).is_ok());
        assert!(GPT::find_from(&mut fs::File::open(DISK2).unwrap()).is_ok());
    }

    #[test]
    fn input_too_short() {
        let mut empty = io::Cursor::new(vec![1; 5]);
        assert!(matches!(
            GPT::read_from(&mut empty, 512).expect_err("Should fail on short input"),
            Error::InvalidSignature
        ));
    }

    #[test]
    fn find_backup() {
        fn test(path: &str, ss: u64) {
            let mut cur = io::Cursor::new(fs::read(path).unwrap());
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            assert_eq!(gpt.header.partition_entry_lba, 2);
            gpt.header.crc32_checksum = 1;
            cur.seek(SeekFrom::Start(gpt.sector_size)).unwrap();
            cur.write_all(&gpt.header.to_bytes()).unwrap();
            let maybe_gpt = GPT::read_from(&mut cur, gpt.sector_size);
            assert!(maybe_gpt.is_ok());
            let gpt = maybe_gpt.unwrap();
            let end = cur.seek(SeekFrom::End(0)).unwrap() / gpt.sector_size - 1;
            assert_eq!(gpt.header.primary_lba, end);
            assert_eq!(gpt.header.backup_lba, 1);
            assert_eq!(
                gpt.header.partition_entry_lba,
                gpt.header.last_usable_lba + 1
            );
            assert!(GPT::find_from(&mut cur).is_ok());
        }

        test(DISK1, 512);
        test(DISK2, 4096);
    }

    #[test]
    fn add_partition_left() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();
        gpt.align = 1;

        assert_eq!(gpt.find_first_place(10000), None);
        assert_eq!(gpt.find_first_place(4), Some(44));
        assert_eq!(gpt.find_first_place(8), Some(53));
    }

    #[test]
    fn error_messages() {
        use std::error::Error as _;

        let cases = [
            (
                Error::Io(io::ErrorKind::UnexpectedEof.into()),
                "generic I/O error",
            ),
            (Error::InvalidSignature, "invalid signature"),
            (Error::InvalidRevision, "invalid revision"),
            (Error::InvalidHeaderSize, "invalid header size"),
            (
                Error::InvalidPartitionEntrySize(256),
                "invalid partition entry size: 256",
            ),
            (
                Error::InvalidPartitionEntryArraySize(134_217_728),
                "partition entry array too large: 134217728 bytes",
            ),
            (
                Error::InvalidPartitionEntryCount(127, 128),
                "invalid number of partition entries (127 != 128)",
            ),
            (
                Error::InvalidChecksum(1, 2),
                "corrupted CRC32 checksum (1 != 2)",
            ),
            (
                Error::InvalidPartitionEntryArrayChecksum(3, 4),
                "corrupted partition entry array CRC32 checksum (3 != 4)",
            ),
            (
                Error::ReadError(
                    Box::new(Error::InvalidSignature),
                    Box::new(Error::InvalidRevision),
                ),
                "could not read primary header (invalid signature) nor backup header \
                (invalid revision)",
            ),
            (Error::NoSpaceLeft, "no space left"),
            (Error::ConflictPartitionGUID, "conflict of partition GUIDs"),
            (
                Error::InvalidPartitionBoundaries,
                "invalid partition boundaries: partitions must have positive size, must not \
                overlap, and must fit within the disk",
            ),
            (
                Error::InvalidPartitionNumber(1),
                "invalid partition number: 1",
            ),
            (Error::UnusedPartition, "unused partition"),
            (Error::PartitionNotFound, "partition not found"),
            (Error::Overflow, "an arithmetic operation overflowed"),
        ];
        for (err, msg) in cases {
            assert_eq!(err.to_string(), msg);
            assert_eq!(err.source().is_some(), matches!(err, Error::Io(_)));
        }

        let err = Error::from(io::Error::from(io::ErrorKind::UnexpectedEof));
        let source = err.source().unwrap().downcast_ref::<io::Error>().unwrap();
        assert_eq!(source.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn truncated_input_returns_unexpected_eof() {
        fn test(path: &str, ss: u64) {
            let mut f = fs::File::open(path).unwrap();
            f.seek(SeekFrom::Start(ss)).unwrap();
            let header = GPTHeader::read_from(&mut f).unwrap();

            // A header that is 91 bytes long.
            let mut data = vec![0; ss as usize + 91];
            data[ss as usize..].copy_from_slice(&header.to_bytes()[..91]);
            match GPTHeader::read_from(&mut io::Cursor::new(&mut data[ss as usize..])) {
                Err(Error::Io(err)) => assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof),
                x => panic!("unexpected result: {x:?}"),
            }

            // A partition entry that is 127 bytes long.
            let mut data = [0; 127];
            data.copy_from_slice(&GPTPartitionEntry::empty().to_bytes()[..127]);
            match GPTPartitionEntry::read_from(&mut io::Cursor::new(&mut data[..])) {
                Err(Error::Io(err)) => assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof),
                x => panic!("unexpected result: {x:?}"),
            }
        }

        test(DISK1, 512);
        test(DISK2, 4096);
    }

    #[test]
    fn add_partition_left_aligned() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();

        gpt.align = 10000;
        assert_eq!(gpt.find_first_place(1), None);
        gpt.align = 4;
        assert_eq!(gpt.find_first_place(4), Some(44));
        gpt.align = 6;
        assert_eq!(gpt.find_first_place(4), Some(54));
    }

    #[test]
    fn add_partition_right() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK2).unwrap()).unwrap();
        gpt.align = 1;

        assert_eq!(gpt.find_last_place(10000), None);
        assert_eq!(gpt.find_last_place(5), Some(90));
        assert_eq!(gpt.find_last_place(20), Some(50));
    }

    #[test]
    fn add_partition_right_aligned() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK2).unwrap()).unwrap();

        gpt.align = 10000;
        assert_eq!(gpt.find_last_place(1), None);
        gpt.align = 4;
        assert_eq!(gpt.find_last_place(5), Some(88));
        gpt.align = 8;
        assert_eq!(gpt.find_last_place(20), Some(48));

        // NOTE: special case where there is just enough space but it's not aligned
        gpt.align = 1;
        assert_eq!(gpt.find_last_place(54), Some(16));
        assert_eq!(gpt.find_last_place(55), None);
        gpt.align = 10;
        assert_eq!(gpt.find_last_place(54), None);
    }

    #[test]
    fn add_partition_optimal() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK2).unwrap()).unwrap();
        gpt.align = 1;

        assert_eq!(gpt.find_optimal_place(10000), None);
        assert_eq!(gpt.find_optimal_place(5), Some(80));
        assert_eq!(gpt.find_optimal_place(20), Some(16));
    }

    #[test]
    fn add_partition_optimal_aligned() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK2).unwrap()).unwrap();

        gpt.align = 10000;
        assert_eq!(gpt.find_optimal_place(1), None);
        gpt.align = 6;
        assert_eq!(gpt.find_optimal_place(5), Some(84));
        gpt.align = 9;
        assert_eq!(gpt.find_optimal_place(20), Some(18));
    }

    #[test]
    fn sort_partitions() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();
        gpt.align = 1;

        let starting_lba = gpt.find_first_place(4).unwrap();
        gpt[10] = GPTPartitionEntry {
            starting_lba,
            ending_lba: starting_lba + 3,
            attribute_bits: 0,
            partition_type_guid: [1; 16],
            partition_name: "Baz".into(),
            unique_partition_guid: [1; 16],
        };

        assert_eq!(
            gpt.iter()
                .filter(|(_, x)| x.is_used())
                .map(|(i, x)| (i, x.partition_name.as_str()))
                .collect::<Vec<_>>(),
            vec![(1, "Foo"), (2, "Bar"), (10, "Baz")]
        );
        gpt.sort();
        assert_eq!(
            gpt.iter()
                .filter(|(_, x)| x.is_used())
                .map(|(i, x)| (i, x.partition_name.as_str()))
                .collect::<Vec<_>>(),
            vec![(1, "Foo"), (2, "Baz"), (3, "Bar")]
        );
    }

    #[test]
    fn add_partition_on_unsorted_table() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();
        gpt.align = 1;

        let starting_lba = gpt.find_first_place(4).unwrap();
        gpt.partitions[10] = GPTPartitionEntry {
            starting_lba,
            ending_lba: starting_lba + 3,
            attribute_bits: 0,
            partition_type_guid: [1; 16],
            partition_name: "Baz".into(),
            unique_partition_guid: [1; 16],
        };

        assert_eq!(gpt.find_first_place(8), Some(53));
    }

    #[test]
    fn write_from_primary() {
        fn test(path: &str, ss: u64) {
            let mut f = fs::File::open(path).unwrap();
            let len = f.seek(SeekFrom::End(0)).unwrap();
            let data = vec![0; len as usize];
            let mut cur = io::Cursor::new(data);
            let mut gpt = GPT::read_from(&mut f, ss).unwrap();
            let backup_lba = gpt.header.backup_lba;
            gpt.write_into(&mut cur).unwrap();
            assert!(GPT::read_from(&mut cur, ss).is_ok());

            gpt.header.crc32_checksum = 1;
            cur.seek(SeekFrom::Start(ss)).unwrap();
            cur.write_all(&gpt.header.to_bytes()).unwrap();
            let maybe_gpt = GPT::read_from(&mut cur, ss);
            assert!(maybe_gpt.is_ok());
            let gpt = maybe_gpt.unwrap();
            assert_eq!(gpt.header.primary_lba, backup_lba);
            assert_eq!(gpt.header.backup_lba, 1);
        }

        test(DISK1, 512);
        test(DISK2, 4096);
    }

    #[test]
    fn write_from_backup() {
        fn test(path: &str, ss: u64) {
            let mut cur = io::Cursor::new(fs::read(path).unwrap());
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            let primary = gpt.clone();
            gpt.header.crc32_checksum = 1;
            let backup_lba = gpt.header.backup_lba;
            cur.seek(SeekFrom::Start(ss)).unwrap();
            cur.write_all(&gpt.header.to_bytes()).unwrap();
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            assert!(!gpt.is_primary());
            assert!(gpt.is_backup());
            let partition_entry_lba = gpt.header.partition_entry_lba;
            let first_usable_lba = gpt.header.first_usable_lba;
            let last_usable_lba = gpt.header.last_usable_lba;
            let primary_header = gpt.write_into(&mut cur).unwrap();
            assert!(primary_header.is_primary());
            assert!(!primary_header.is_backup());
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            assert_eq!(gpt.header.primary_lba, 1);
            assert_eq!(gpt.header.backup_lba, backup_lba);
            assert_eq!(gpt.header.partition_entry_lba, 2);
            assert_eq!(gpt.header.first_usable_lba, first_usable_lba);
            assert_eq!(gpt.header.last_usable_lba, last_usable_lba);
            assert_eq!(primary, gpt);

            gpt.header.crc32_checksum = 1;
            cur.seek(SeekFrom::Start(ss)).unwrap();
            cur.write_all(&gpt.header.to_bytes()).unwrap();
            let maybe_gpt = GPT::read_from(&mut cur, ss);
            assert!(maybe_gpt.is_ok());
            let gpt = maybe_gpt.unwrap();
            assert_eq!(gpt.header.primary_lba, backup_lba);
            assert_eq!(gpt.header.backup_lba, 1);
            assert_eq!(gpt.header.partition_entry_lba, partition_entry_lba);
        }

        test(DISK1, 512);
        test(DISK2, 4096);
    }

    #[test]
    fn write_with_changes() {
        fn test(path: &str, ss: u64) {
            let mut f = fs::File::open(path).unwrap();
            let len = f.seek(SeekFrom::End(0)).unwrap();
            let data = vec![0; len as usize];
            let mut cur = io::Cursor::new(data);
            let mut gpt = GPT::read_from(&mut f, ss).unwrap();
            let backup_lba = gpt.header.backup_lba;

            assert!(gpt.remove(1).is_ok());
            gpt.write_into(&mut cur).unwrap();
            let maybe_gpt = GPT::read_from(&mut cur, ss);
            assert!(maybe_gpt.is_ok(), "{:?}", maybe_gpt.err());

            gpt.header.crc32_checksum = 1;
            cur.seek(SeekFrom::Start(ss)).unwrap();
            cur.write_all(&gpt.header.to_bytes()).unwrap();
            let maybe_gpt = GPT::read_from(&mut cur, ss);
            assert!(maybe_gpt.is_ok());
            let gpt = maybe_gpt.unwrap();
            assert_eq!(gpt.header.primary_lba, backup_lba);
            assert_eq!(gpt.header.backup_lba, 1);
        }

        test(DISK1, 512);
        test(DISK2, 4096);
    }

    #[test]
    #[allow(clippy::manual_swap)]
    fn write_invalid_boundaries() {
        fn test(path: &str, ss: u64) {
            let mut cur = io::Cursor::new(fs::read(path).unwrap());
            // start before first_usable_lba
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            gpt[1].starting_lba = gpt.header.first_usable_lba - 1;
            gpt.write_into(&mut cur).unwrap_err();
            // end before start
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            let start = gpt[1].starting_lba;
            gpt[1].starting_lba = gpt[1].ending_lba;
            gpt[1].ending_lba = start;
            gpt.write_into(&mut cur).unwrap_err();
            // overlap
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            gpt[1].ending_lba = gpt[2].starting_lba;
            gpt.write_into(&mut cur).unwrap_err();
            // end after last_usable_lba
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            gpt[2].ending_lba = gpt.header.last_usable_lba + 1;
            gpt.write_into(&mut cur).unwrap_err();
            // round-trip, everything valid
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            gpt.write_into(&mut cur).unwrap();
        }
        test(DISK1, 512);
        test(DISK2, 4096);
    }

    #[test]
    fn get_maximum_partition_size_on_empty_disk() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();
        gpt.align = 1;

        for i in 1..=gpt.header.number_of_partition_entries {
            assert!(gpt.remove(i).is_ok());
        }

        assert_eq!(gpt.get_maximum_partition_size().ok(), Some(33));
    }

    #[test]
    fn get_maximum_partition_size_on_disk_full() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();
        gpt.align = 1;

        for partition in gpt.partitions.iter_mut().skip(1) {
            partition.partition_type_guid = [0; 16];
        }
        gpt.partitions[0].starting_lba = gpt.header.first_usable_lba;
        gpt.partitions[0].ending_lba = gpt.header.last_usable_lba;

        assert!(gpt.get_maximum_partition_size().is_err());
    }

    #[test]
    fn get_maximum_partition_size_on_empty_disk_and_aligned() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();

        for i in 1..=gpt.header.number_of_partition_entries {
            assert!(gpt.remove(i).is_ok());
        }

        gpt.align = 10;
        assert_eq!(gpt.get_maximum_partition_size().ok(), Some(20));
        gpt.align = 6;
        assert_eq!(gpt.get_maximum_partition_size().ok(), Some(30));
    }

    #[test]
    fn create_new_gpt() {
        fn test(path: &str, ss: u64) {
            let mut f = fs::File::open(path).unwrap();
            let gpt1 = GPT::read_from(&mut f, ss).unwrap();
            let gpt2 = GPT::new_from(&mut f, ss, [1; 16]).unwrap();
            assert_eq!(gpt2.header.backup_lba, gpt1.header.backup_lba);
            assert_eq!(gpt2.header.last_usable_lba, gpt1.header.last_usable_lba);
            assert_eq!(gpt2.header.first_usable_lba, gpt1.header.first_usable_lba);
        }

        test(DISK1, 512);
        test(DISK2, 4096);
    }

    #[test]
    fn determine_partition_alignment_no_partition() {
        fn test(ss: u64) {
            let data = vec![0; ss as usize * DEFAULT_ALIGN as usize * 10];
            let mut cur = io::Cursor::new(data);
            let mut gpt = GPT::new_from(&mut cur, ss, [1; 16]).unwrap();
            assert_eq!(gpt.align, DEFAULT_ALIGN);
            gpt.write_into(&mut cur).unwrap();
            let gpt = GPT::read_from(&mut cur, ss).unwrap();
            assert_eq!(gpt.align, DEFAULT_ALIGN);
        }

        test(512);
        test(4096);
    }

    #[test]
    fn determine_partition_alignment() {
        fn test(ss: u64, align: u64) {
            let data = vec![0; ss as usize * align as usize * 21];
            let mut cur = io::Cursor::new(data);
            let mut gpt = GPT::new_from(&mut cur, ss, [1; 16]).unwrap();
            gpt[1] = GPTPartitionEntry {
                attribute_bits: 0,
                ending_lba: 6 * align,
                partition_name: "".into(),
                partition_type_guid: [1; 16],
                // start at least at first_usable_lba in smallest case
                starting_lba: 5 * align,
                unique_partition_guid: [1; 16],
            };
            gpt[2] = GPTPartitionEntry {
                attribute_bits: 0,
                ending_lba: 16 * align,
                partition_name: "".into(),
                partition_type_guid: [1; 16],
                starting_lba: 8 * align,
                unique_partition_guid: [2; 16],
            };
            gpt.write_into(&mut cur).unwrap();
            let gpt = GPT::read_from(&mut cur, ss).unwrap();
            assert_eq!(gpt.align, align);
        }

        test(512, 8); // 4096 bytes
        test(512, 2048); // 1MB
        test(512, 2048 * 4); // 4MB
        test(4096, 8);
        test(4096, 2048);
        test(4096, 2048 * 4);
    }

    #[test]
    fn determine_partition_alignment_full_disk() {
        fn test(ss: u64) {
            let data = vec![0; ss as usize * 100];
            let mut cur = io::Cursor::new(data);
            let mut gpt = GPT::new_from(&mut cur, ss, [1; 16]).unwrap();
            gpt[1] = GPTPartitionEntry {
                attribute_bits: 0,
                ending_lba: gpt.header.last_usable_lba,
                partition_name: "".into(),
                partition_type_guid: [1; 16],
                starting_lba: gpt.header.first_usable_lba,
                unique_partition_guid: [1; 16],
            };
            gpt.write_into(&mut cur).unwrap();
            let gpt = GPT::read_from(&mut cur, ss).unwrap();
            assert_eq!(gpt.align, 1);

            let mut gpt = GPT::new_from(&mut cur, ss, [1; 16]).unwrap();
            gpt[1] = GPTPartitionEntry {
                attribute_bits: 0,
                ending_lba: gpt.header.last_usable_lba,
                partition_name: "".into(),
                partition_type_guid: [1; 16],
                starting_lba: gpt.header.first_usable_lba + 1,
                unique_partition_guid: [1; 16],
            };
            gpt.write_into(&mut cur).unwrap();
            let gpt = GPT::read_from(&mut cur, ss).unwrap();
            assert_eq!(gpt.align, gpt.header.first_usable_lba + 1);
        }

        test(512);
        test(4096);
    }

    #[test]
    fn writing_protective_mbr() {
        fn test(ss: u64) {
            let data = vec![2; ss as usize * 100];
            let mut cur = io::Cursor::new(data);
            GPT::write_protective_mbr_into(&mut cur, ss).unwrap();
            let data = cur.get_ref();

            assert_eq!(data[510], 0x55);
            assert_eq!(data[511], 0xaa);
            assert_eq!(data[446 + 4], 0xee);
            for (i, x) in data.iter().enumerate() {
                if !(446..512).contains(&i) {
                    assert_eq!(*x, 2);
                }
            }

            cur.seek(SeekFrom::Start(446 + 8)).unwrap();
            let mut bytes = [0; 4];
            cur.read_exact(&mut bytes).unwrap();
            let first_lba = u32::from_le_bytes(bytes);
            cur.read_exact(&mut bytes).unwrap();
            let sectors = u32::from_le_bytes(bytes);
            assert_eq!(first_lba, 1);
            assert_eq!(sectors, 99);
        }

        test(512);
        test(4096);
    }

    #[test]
    fn read_from_smaller_disk_and_write_to_bigger_disk() {
        fn test(path: &str, ss: u64) {
            let mut f = fs::File::open(path).unwrap();
            let len = f.seek(SeekFrom::End(0)).unwrap();
            let gpt1 = GPT::read_from(&mut f, ss).unwrap();
            let data = vec![0; len as usize * 2];
            let mut cur = io::Cursor::new(data);
            gpt1.clone().write_into(&mut cur).unwrap();
            let gpt2 = GPT::read_from(&mut cur, ss).unwrap();
            assert_eq!(gpt1, gpt2);
        }

        test(DISK1, 512);
        test(DISK2, 4096);
    }

    #[test]
    fn read_label_with_trailing_garbage_in_names() {
        fn test(path: &str, ss: u64) {
            let mut f = fs::File::open(path).unwrap();
            let gpt = GPT::read_from(&mut f, ss);
            // CRC check passes with trailing garbage within partition names
            assert!(gpt.is_ok());
            // Trailing garbage data are visible using hexdump
            assert_eq!(
                gpt.unwrap()
                    .partitions
                    .get(1)
                    .unwrap()
                    .partition_name
                    .as_str(),
                "Name with garbage"
            );
        }
        test(DISK3, 512);
        test(DISK4, 4096);
    }

    #[test]
    fn gpt_read_from_on_image_with_corrupted_partition_entry_array_crc32_in_primary() {
        let mut f = fs::File::open(DISK5).unwrap();
        let gpt = GPT::read_from(&mut f, 512).unwrap();
        // Verify that the backup header was actually used, not the primary.
        // In a backup header, primary_lba points to the backup's own location
        // (last sector) and backup_lba points back to 1 (the primary's location).
        let end = f.seek(SeekFrom::End(0)).unwrap() / 512 - 1;
        assert_eq!(gpt.header.primary_lba, end);
        assert_eq!(gpt.header.backup_lba, 1);
        assert_eq!(
            gpt.header.partition_entry_lba,
            gpt.header.last_usable_lba + 1
        );
    }

    #[test]
    fn header_and_partition_entry_bytes_round_trip() {
        fn test(path: &str, ss: u64) {
            let data = fs::read(path).unwrap();

            let header_bytes: [u8; 92] = data[ss as usize..][..92].try_into().unwrap();
            let header = GPTHeader::from_bytes(&header_bytes).unwrap();
            assert_eq!(header.to_bytes(), header_bytes);

            let mut offset = header.partition_entry_lba as usize * ss as usize;
            for _ in 0..header.number_of_partition_entries {
                let entry_bytes: [u8; 128] = data[offset..][..128].try_into().unwrap();
                let entry = GPTPartitionEntry::from_bytes(&entry_bytes);
                assert_eq!(entry.to_bytes(), entry_bytes);
                offset += 128;
            }
        }

        test(DISK1, 512);
        test(DISK2, 4096);
        test(DISK3, 512);
        test(DISK4, 4096);
    }

    #[test]
    fn partition_name_bytes_round_trip() {
        let name: PartitionName = "A Robot Named Fight!".into();
        assert_eq!(PartitionName::from_bytes(&name.to_bytes()), name);

        // A name that fills the buffer exactly has no null terminator.
        let name: PartitionName = "012345678901234567890123456789012345".into();
        assert_eq!(PartitionName::from_bytes(&name.to_bytes()), name);
    }

    #[test]
    fn read_partitions_rejects_mismatched_entry_count() {
        let mut f = fs::File::open(DISK1).unwrap();
        f.seek(SeekFrom::Start(512)).unwrap();
        let header = GPTHeader::read_from(&mut f).unwrap();
        let mut partitions = header.read_partitions(&mut f, 512).unwrap();
        partitions.pop();
        assert!(matches!(
            header.generate_partition_entry_array_crc32(&partitions),
            Err(Error::InvalidPartitionEntryCount(127, 128))
        ));
    }

    #[test]
    fn from_bytes_rejects_unsupported_entry_size() {
        fn test(path: &str, ss: u64) {
            let data = fs::read(path).unwrap();
            let bytes: [u8; 92] = data[ss as usize..][..92].try_into().unwrap();
            let header = GPTHeader::from_bytes(&bytes).unwrap();

            // 256-byte entries are valid in the specification but not supported by gptman.
            let mut header = header;
            header.size_of_partition_entry = 256;
            header.update_crc32_checksum();

            match GPTHeader::from_bytes(&header.to_bytes()) {
                Err(Error::InvalidPartitionEntrySize(256)) => {}
                x => panic!("unexpected result: {x:?}"),
            }
        }

        test(DISK1, 512);
        test(DISK2, 4096);
    }

    #[test]
    fn from_bytes_rejects_oversized_partition_entry_array() {
        fn test(path: &str, ss: u64) {
            let data = fs::read(path).unwrap();
            let bytes: [u8; 92] = data[ss as usize..][..92].try_into().unwrap();

            // NOTE: 1 048 576 entries of 128 bytes is 128 MiB, well over the limit.
            let mut header = GPTHeader::from_bytes(&bytes).unwrap();
            header.number_of_partition_entries = 1_048_576;
            header.update_crc32_checksum();

            match GPTHeader::from_bytes(&header.to_bytes()) {
                Err(Error::InvalidPartitionEntryArraySize(134_217_728)) => {}
                x => panic!("unexpected result: {x:?}"),
            }
        }

        test(DISK1, 512);
        test(DISK2, 4096);
    }
}

#[cfg(doctest)]
mod test_readme {
    // for Rust < 1.54
    // https://blog.guillaume-gomez.fr/articles/2021-08-03+Improvements+for+%23%5Bdoc%5D+attributes+in+Rust
    macro_rules! check_doc {
        ($x:expr) => {
            #[doc = $x]
            extern "C" {}
        };
    }
    check_doc!(include_str!("../README.md"));
}
