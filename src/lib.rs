//! A library that allows managing GUID partition tables.
//!
//! # Examples
//!
//! Reading all the partitions of a disk:
//!
//! ```
//! let mut f = std::fs::File::open("tests/fixtures/disk1.img")
//!     .expect("could not open disk");
//! let gpt = gptman::GPT::find_from(&mut f)
//!     .expect("could not find GPT");
//!
//! println!("Disk GUID: {:?}", gpt.header.disk_guid);
//!
//! for (i, p) in gpt.iter() {
//!     if p.is_used() {
//!         println!("Partition #{}: type = {:?}, size = {} bytes, starting lba = {}",
//!             i,
//!             p.partition_type_guid,
//!             p.size().unwrap() * gpt.sector_size,
//!             p.starting_lba);
//!     }
//! }
//! ```
//!
//! Creating new partitions:
//!
//! ```
//! let mut f = std::fs::File::open("tests/fixtures/disk1.img")
//!     .expect("could not open disk");
//! let mut gpt = gptman::GPT::find_from(&mut f)
//!     .expect("could not find GPT");
//!
//! let free_partition_number = gpt.iter().find(|(i, p)| p.is_unused()).map(|(i, _)| i)
//!     .expect("no more places available");
//! let size = gpt.get_maximum_partition_size()
//!     .expect("no more space available");
//! let starting_lba = gpt.find_optimal_place(size)
//!     .expect("could not find a place to put the partition");
//! let ending_lba = starting_lba + size - 1;
//!
//! gpt[free_partition_number] = gptman::GPTPartitionEntry {
//!     partition_type_guid: [0xff; 16],
//!     unique_partition_guid: [0xff; 16],
//!     starting_lba,
//!     ending_lba,
//!     attribute_bits: 0,
//!     partition_name: "A Robot Named Fight!".into(),
//! };
//! ```
//!
//! Creating a new partition table with one entry that fills the entire disk:
//!
//! ```
//! let ss = 512;
//! let data = vec![0; 100 * ss as usize];
//! let mut cur = std::io::Cursor::new(data);
//! let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
//!     .expect("could not create partition table");
//!
//! gpt[1] = gptman::GPTPartitionEntry {
//!     partition_type_guid: [0xff; 16],
//!     unique_partition_guid: [0xff; 16],
//!     starting_lba: gpt.header.first_usable_lba,
//!     ending_lba: gpt.header.last_usable_lba,
//!     attribute_bits: 0,
//!     partition_name: "A Robot Named Fight!".into(),
//! };
//! ```

#![deny(missing_docs)]

use bincode::{deserialize_from, serialize, serialize_into};
use crc::{Crc, CRC_32_ISO_HDLC};
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeTuple;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::{Index, IndexMut};
use thiserror::Error;

/// Linux specific helpers
#[cfg(target_os = "linux")]
pub mod linux;

const DEFAULT_ALIGN: u64 = 2048;
const MAX_ALIGN: u64 = 16384;

/// An error that can be produced while reading, writing or managing a GPT.
#[derive(Debug, Error)]
pub enum Error {
    /// Derialization errors.
    #[error("deserialization failed")]
    Deserialize(#[from] bincode::Error),
    /// I/O errors.
    #[error("generic I/O error")]
    Io(#[from] io::Error),
    /// An error that occurs when the signature of the GPT isn't what would be expected ("EFI
    /// PART").
    #[error("invalid signature")]
    InvalidSignature,
    /// An error that occurs when the revision of the GPT isn't what would be expected (00 00 01
    /// 00).
    #[error("invalid revision")]
    InvalidRevision,
    /// An error that occurs when the header's size (in bytes) isn't what would be expected (92).
    #[error("invalid header size")]
    InvalidHeaderSize,
    /// An error that occurs when the CRC32 checksum of the header doesn't match the expected
    /// checksum for the actual header.
    #[error("corrupted CRC32 checksum ({0} != {1})")]
    InvalidChecksum(u32, u32),
    /// An error that occurs when the CRC32 checksum of the partition entries array doesn't match
    /// the expected checksum for the actual partition entries array.
    #[error("corrupted partition entry array CRC32 checksum ({0} != {1})")]
    InvalidPartitionEntryArrayChecksum(u32, u32),
    /// An error that occurs when reading a GPT from a file did not succeeded.
    ///
    /// The first argument is the error that occurred when trying to read the primary header.
    /// The second argument is the error that occurred when trying to read the backup header.
    #[error("could not read primary header ({0}) nor backup header ({1})")]
    ReadError(Box<Error>, Box<Error>),
    /// An error that occurs when there is not enough space left on the table to continue.
    #[error("no space left")]
    NoSpaceLeft,
    /// An error that occurs when there are partitions with the same GUID in the same array.
    #[error("conflict of partition GUIDs")]
    ConflictPartitionGUID,
    /// An error that occurs when a partition has an invalid boundary.
    /// The end sector must be greater or equal to the start sector of the partition.
    /// Partitions must fit within the disk and must not overlap.
    #[error(
        "invalid partition boundaries: partitions must have positive size, must not overlap, \
        and must fit within the disk"
    )]
    InvalidPartitionBoundaries,
    /// An error that occurs when the user provide an invalid partition number.
    /// The partition number must be between 1 and `number_of_partition_entries` (usually 128)
    /// included.
    #[error("invalid partition number: {0}")]
    InvalidPartitionNumber(u32),
    /// An operation that required to find a partition, was unable to find that partition.
    #[error("partition not found")]
    PartitionNotFound,
}

/// The result of reading, writing or managing a GPT.
pub type Result<T> = std::result::Result<T, Error>;

/// A GUID Partition Table header as describe on
/// [Wikipedia's page](https://en.wikipedia.org/wiki/GUID_Partition_Table#Partition_table_header_(LBA_1)).
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
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

    /// Attempt to read a GPT header from a reader.
    ///
    /// # Implementation notes
    ///
    /// The field `last_usable_lba` is not updated to reflect the actual size of the disk. You must
    /// do this yourself by calling `update_from`.
    pub fn read_from<R: ?Sized>(mut reader: &mut R) -> Result<GPTHeader>
    where
        R: Read + Seek,
    {
        let gpt: GPTHeader = deserialize_from(&mut reader)?;

        if String::from_utf8_lossy(&gpt.signature) != "EFI PART" {
            return Err(Error::InvalidSignature);
        }

        if gpt.revision != [0x00, 0x00, 0x01, 0x00] {
            return Err(Error::InvalidRevision);
        }

        if gpt.header_size != 92 {
            return Err(Error::InvalidHeaderSize);
        }

        let sum = gpt.generate_crc32_checksum();
        if gpt.crc32_checksum != sum {
            return Err(Error::InvalidChecksum(gpt.crc32_checksum, sum));
        }

        Ok(gpt)
    }

    /// Write the GPT header into a writer. This operation will update the CRC32 checksums of the
    /// current struct and seek at the location `primary_lba` before trying to write to disk.
    pub fn write_into<W: ?Sized>(
        &mut self,
        mut writer: &mut W,
        sector_size: u64,
        partitions: &[GPTPartitionEntry],
    ) -> Result<()>
    where
        W: Write + Seek,
    {
        self.update_partition_entry_array_crc32(partitions);
        self.update_crc32_checksum();

        writer.seek(SeekFrom::Start(self.primary_lba * sector_size))?;
        serialize_into(&mut writer, &self)?;

        for i in 0..self.number_of_partition_entries {
            writer.seek(SeekFrom::Start(
                self.partition_entry_lba * sector_size
                    + u64::from(i) * u64::from(self.size_of_partition_entry),
            ))?;
            serialize_into(&mut writer, &partitions[i as usize])?;
        }

        Ok(())
    }

    /// Generate the CRC32 checksum of the partition header only.
    pub fn generate_crc32_checksum(&self) -> u32 {
        let mut clone = self.clone();
        clone.crc32_checksum = 0;
        let data = serialize(&clone).expect("could not serialize");
        assert_eq!(data.len() as u32, clone.header_size);

        Crc::<u32>::new(&CRC_32_ISO_HDLC).checksum(&data)
    }

    /// Update the CRC32 checksum of this header.
    pub fn update_crc32_checksum(&mut self) {
        self.crc32_checksum = self.generate_crc32_checksum();
    }

    /// Generate the CRC32 checksum of the partition entry array.
    pub fn generate_partition_entry_array_crc32(&self, partitions: &[GPTPartitionEntry]) -> u32 {
        let mut clone = self.clone();
        clone.partition_entry_array_crc32 = 0;
        let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
        let mut digest = crc.digest();
        let mut wrote = 0;
        for x in partitions {
            let data = serialize(&x).expect("could not serialize");
            digest.update(&data);
            wrote += data.len();
        }
        assert_eq!(
            wrote as u32,
            clone.size_of_partition_entry * clone.number_of_partition_entries
        );

        digest.finalize()
    }

    /// Update the CRC32 checksum of the partition entry array.
    pub fn update_partition_entry_array_crc32(&mut self, partitions: &[GPTPartitionEntry]) {
        self.partition_entry_array_crc32 = self.generate_partition_entry_array_crc32(partitions);
    }

    /// Updates the header to match the specifications of the seeker given in argument.
    /// `first_usable_lba`, `last_usable_lba`, `primary_lba`, `backup_lba`, `partition_entry_lba`
    /// will be updated after this operation.
    pub fn update_from<S: ?Sized>(&mut self, seeker: &mut S, sector_size: u64) -> Result<()>
    where
        S: Seek,
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
pub struct PartitionName(String);

impl PartitionName {
    /// Extracts a string slice containing the entire `PartitionName`.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for PartitionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for PartitionName {
    fn from(value: &str) -> PartitionName {
        PartitionName(value.to_string())
    }
}

struct UTF16LEVisitor;

impl<'de> Visitor<'de> for UTF16LEVisitor {
    type Value = PartitionName;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("36 UTF-16LE code units (72 bytes)")
    }

    fn visit_seq<A>(self, mut seq: A) -> std::result::Result<PartitionName, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut v = Vec::new();
        let mut end = false;
        loop {
            match seq.next_element()? {
                Some(0) => end = true,
                Some(x) if !end => v.push(x),
                Some(_) => {}
                None => break,
            }
        }

        Ok(PartitionName(String::from_utf16_lossy(&v)))
    }
}

impl<'de> Deserialize<'de> for PartitionName {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_tuple(36, UTF16LEVisitor)
    }
}

impl Serialize for PartitionName {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = self.0.encode_utf16();
        let mut seq = serializer.serialize_tuple(36)?;
        for x in s.chain([0].iter().cycle().cloned()).take(36) {
            seq.serialize_element(&x)?;
        }
        seq.end()
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
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
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
    pub fn read_from<R: ?Sized>(mut reader: &mut R) -> bincode::Result<GPTPartitionEntry>
    where
        R: Read,
    {
        deserialize_from(&mut reader)
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
    pub fn read_from<R: ?Sized>(mut reader: &mut R, sector_size: u64) -> Result<GPT>
    where
        R: Read + Seek,
    {
        use self::Error::*;

        reader.seek(SeekFrom::Start(sector_size))?;
        let header = GPTHeader::read_from(&mut reader).or_else(|primary_err| {
            let len = reader.seek(SeekFrom::End(0))?;
            reader.seek(SeekFrom::Start((len / sector_size - 1) * sector_size))?;

            GPTHeader::read_from(&mut reader).map_err(|backup_err| {
                match (primary_err, backup_err) {
                    (InvalidSignature, InvalidSignature) => InvalidSignature,
                    (x, y) => Error::ReadError(Box::new(x), Box::new(y)),
                }
            })
        })?;

        let mut partitions = Vec::with_capacity(header.number_of_partition_entries as usize);
        for i in 0..header.number_of_partition_entries {
            reader.seek(SeekFrom::Start(
                header.partition_entry_lba * sector_size
                    + u64::from(i) * u64::from(header.size_of_partition_entry),
            ))?;
            partitions.push(GPTPartitionEntry::read_from(&mut reader)?);
        }

        let sum = header.generate_partition_entry_array_crc32(&partitions);
        if header.partition_entry_array_crc32 != sum {
            return Err(Error::InvalidPartitionEntryArrayChecksum(
                header.partition_entry_array_crc32,
                sum,
            ));
        }

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
    pub fn find_from<R: ?Sized>(mut reader: &mut R) -> Result<GPT>
    where
        R: Read + Seek,
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
    pub fn write_into<W: ?Sized>(&mut self, mut writer: &mut W) -> Result<GPTHeader>
    where
        W: Write + Seek,
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
    /// This function will return a vector of tuple with on the left: the starting LBA of the free
    /// spot; and on the right: the size (in sectors) of the free spot.
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
        let mut positions = Vec::new();
        positions.push(self.header.first_usable_lba - 1);
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
            .filter(|(_, l)| *l >= size)
            .last()
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

    /// Remove a partiton entry in the array that resides at a given sector.
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
    pub fn write_protective_mbr_into<W: ?Sized>(mut writer: &mut W, sector_size: u64) -> Result<()>
    where
        W: Write + Seek,
    {
        let size = writer.seek(SeekFrom::End(0))? / sector_size - 1;
        writer.seek(SeekFrom::Start(446))?;
        // partition 1
        writer.write_all(&[
            0x00, // status
            0x00, 0x02, 0x00, // CHS address of first absolute sector
            0xee, // partition type
            0xff, 0xff, 0xff, // CHS address of last absolute sector
            0x01, 0x00, 0x00, 0x00, // LBA of first absolute sector
        ])?;
        // number of sectors in partition 1
        serialize_into(
            &mut writer,
            &(if size > u64::from(u32::max_value()) {
                u32::max_value()
            } else {
                size as u32
            }),
        )?;
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
    #![allow(clippy::blacklisted_name)]

    use super::*;
    use std::fs;

    const DISK1: &str = "tests/fixtures/disk1.img";
    const DISK2: &str = "tests/fixtures/disk2.img";

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

                // NOTE: testing that serializing the PartitionName (and the whole struct) works
                let data1 = serialize(&partition).unwrap();
                f.seek(SeekFrom::Start(
                    gpt.partition_entry_lba * ss
                        + u64::from(i) * u64::from(gpt.size_of_partition_entry),
                ))
                .unwrap();
                let mut data2 = vec![0; gpt.size_of_partition_entry as usize];
                f.read_exact(&mut data2).unwrap();
                assert_eq!(data1, data2);

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
            gpt.update_partition_entry_array_crc32(&partitions);
            assert_eq!(gpt.partition_entry_array_crc32, sum);
            assert_eq!(gpt.generate_partition_entry_array_crc32(&partitions), sum);
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
    fn find_backup() {
        fn test(path: &str, ss: u64) {
            let mut cur = io::Cursor::new(fs::read(path).unwrap());
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            assert_eq!(gpt.header.partition_entry_lba, 2);
            gpt.header.crc32_checksum = 1;
            cur.seek(SeekFrom::Start(gpt.sector_size)).unwrap();
            serialize_into(&mut cur, &gpt.header).unwrap();
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
            serialize_into(&mut cur, &gpt.header).unwrap();
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
            serialize_into(&mut cur, &gpt.header).unwrap();
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
            serialize_into(&mut cur, &gpt.header).unwrap();
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
            assert!(maybe_gpt.is_ok(), format!("{:?}", maybe_gpt.err()));

            gpt.header.crc32_checksum = 1;
            cur.seek(SeekFrom::Start(ss)).unwrap();
            serialize_into(&mut cur, &gpt.header).unwrap();
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
                if i < 446 || i >= 512 {
                    assert_eq!(*x, 2);
                }
            }

            cur.seek(SeekFrom::Start(446 + 8)).unwrap();
            let first_lba: u32 = deserialize_from(&mut cur).unwrap();
            let sectors: u32 = deserialize_from(&mut cur).unwrap();
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
}
