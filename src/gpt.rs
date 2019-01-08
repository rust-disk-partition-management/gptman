use bincode::{deserialize_from, serialize, serialize_into};
use crc::{crc32, Hasher32};
use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeTuple, Serializer};
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::{Index, IndexMut};

#[derive(Debug)]
pub enum Error {
    Deserialize(bincode::Error),
    Io(io::Error),
    InvalidSignature,
    InvalidRevision,
    InvalidHeaderSize,
    InvalidChecksum(u32, u32),
    InvalidPartitionEntryArrayChecksum(u32, u32),
    ReadError(Box<Error>, Box<Error>),
    NoSpaceLeft,
    ConflictPartitionGUID,
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Error {
        Error::Deserialize(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match self {
            Deserialize(err) => err.fmt(f),
            Io(err) => err.fmt(f),
            InvalidSignature => write!(f, "invalid signature"),
            InvalidRevision => write!(f, "invalid revision"),
            InvalidHeaderSize => write!(f, "invalid header size"),
            InvalidChecksum(x, y) => write!(f, "corrupted CRC32 checksum ({} != {})", x, y),
            InvalidPartitionEntryArrayChecksum(x, y) => write!(
                f,
                "corrupted partition entry array CRC32 checksum ({} != {})",
                x, y
            ),
            ReadError(x, y) => write!(
                f,
                "could not read primary header ({}) nor backup header ({})",
                x, y
            ),
            NoSpaceLeft => write!(f, "no space left"),
            ConflictPartitionGUID => write!(f, "conflict of partition GUIDs"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Copy, Clone)]
pub struct GPTHeader {
    pub signature: [u8; 8],
    pub revision: [u8; 4],
    pub header_size: u32,
    pub crc32_checksum: u32,
    pub reserved: [u8; 4],
    pub primary_lba: u64,
    pub backup_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub disk_guid: [u8; 16],
    pub partition_entry_lba: u64,
    pub number_of_partition_entries: u32,
    pub size_of_partition_entry: u32,
    pub partition_entry_array_crc32: u32,
}

impl GPTHeader {
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

    pub fn write_into<W: ?Sized>(
        &mut self,
        mut writer: &mut W,
        sector_size: u64,
        partitions: &Vec<GPTPartitionEntry>,
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
                    + i as u64 * self.size_of_partition_entry as u64,
            ))?;
            serialize_into(&mut writer, &partitions[i as usize])?;
        }

        Ok(())
    }

    pub fn generate_crc32_checksum(mut self) -> u32 {
        self.crc32_checksum = 0;
        let data = serialize(&self).expect("could not serialize");
        assert_eq!(data.len() as u32, self.header_size);

        crc32::checksum_ieee(&data)
    }

    pub fn update_crc32_checksum(&mut self) {
        self.crc32_checksum = self.generate_crc32_checksum();
    }

    pub fn generate_partition_entry_array_crc32(
        mut self,
        partitions: &Vec<GPTPartitionEntry>,
    ) -> u32 {
        self.partition_entry_array_crc32 = 0;
        let mut digest = crc32::Digest::new(crc32::IEEE);
        let mut wrote = 0;
        for x in partitions {
            let data = serialize(&x).expect("could not serialize");
            digest.write(&data);
            wrote += data.len();
        }
        assert_eq!(
            wrote as u32,
            self.size_of_partition_entry * self.number_of_partition_entries
        );

        digest.sum32()
    }

    pub fn update_partition_entry_array_crc32(&mut self, partitions: &Vec<GPTPartitionEntry>) {
        self.partition_entry_array_crc32 = self.generate_partition_entry_array_crc32(partitions);
    }

    fn update_from<S: ?Sized>(&mut self, seeker: &mut S, sector_size: u64) -> Result<()>
    where
        S: Seek,
    {
        let partition_array_size = ((self.number_of_partition_entries
            * self.size_of_partition_entry) as f64
            / sector_size as f64)
            .ceil() as u64;
        let len = seeker.seek(SeekFrom::End(0))? / sector_size;
        if self.primary_lba == 1 {
            self.backup_lba = len - 1;
        } else {
            self.primary_lba = len - 1;
        }
        self.last_usable_lba = len - partition_array_size - 1 - 1;
        self.first_usable_lba = self.partition_entry_lba + partition_array_size;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PartitionName(String);

impl PartitionName {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
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

        Ok(PartitionName(String::from_utf16_lossy(&v).to_string()))
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GPTPartitionEntry {
    pub partition_type_guid: [u8; 16],
    pub unique_parition_guid: [u8; 16],
    pub starting_lba: u64,
    pub ending_lba: u64,
    pub attribute_bits: u64,
    pub partition_name: PartitionName,
}

impl GPTPartitionEntry {
    pub fn empty() -> GPTPartitionEntry {
        GPTPartitionEntry {
            partition_type_guid: [0; 16],
            unique_parition_guid: [0; 16],
            starting_lba: 0,
            ending_lba: 0,
            attribute_bits: 0,
            partition_name: "".into(),
        }
    }

    pub fn read_from<R: ?Sized>(mut reader: &mut R) -> bincode::Result<GPTPartitionEntry>
    where
        R: Read + Seek,
    {
        deserialize_from(&mut reader)
    }

    pub fn is_unused(&self) -> bool {
        self.partition_type_guid == [0; 16]
    }

    pub fn is_used(&self) -> bool {
        !self.is_unused()
    }

    pub fn size(&self) -> u64 {
        self.ending_lba - self.starting_lba + 1
    }
}

#[derive(Debug, Clone)]
pub struct GPT {
    pub sector_size: u64,
    pub header: GPTHeader,
    partitions: Vec<GPTPartitionEntry>,
}

impl GPT {
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
        })
    }

    pub fn read_from<R: ?Sized>(mut reader: &mut R, sector_size: u64) -> Result<GPT>
    where
        R: Read + Seek,
    {
        use self::Error::*;

        reader.seek(SeekFrom::Start(sector_size))?;
        let header = GPTHeader::read_from(&mut reader).or_else(|primary_err| {
            let len = reader.seek(SeekFrom::End(0))?;
            reader.seek(SeekFrom::Start((len / sector_size - 1) * sector_size))?;

            GPTHeader::read_from(&mut reader).or_else(|backup_err| {
                match (primary_err, backup_err) {
                    (InvalidSignature, InvalidSignature) => Err(InvalidSignature),
                    (x, y) => Err(Error::ReadError(Box::new(x), Box::new(y))),
                }
            })
        })?;

        let mut partitions = Vec::with_capacity(header.number_of_partition_entries as usize);
        for i in 0..header.number_of_partition_entries {
            reader.seek(SeekFrom::Start(
                header.partition_entry_lba * sector_size
                    + i as u64 * header.size_of_partition_entry as u64,
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

        Ok(GPT {
            sector_size,
            header,
            partitions,
        })
    }

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

    fn check_partition_guids(&self) -> Result<()> {
        let guids: Vec<_> = self
            .partitions
            .iter()
            .filter(|x| x.is_used())
            .map(|x| x.unique_parition_guid)
            .collect();
        if guids.len() != guids.iter().collect::<HashSet<_>>().len() {
            return Err(Error::ConflictPartitionGUID);
        }

        Ok(())
    }

    pub fn write_into<W: ?Sized>(&mut self, mut writer: &mut W) -> Result<()>
    where
        W: Write + Seek,
    {
        self.check_partition_guids()?;
        self.header.update_from(&mut writer, self.sector_size)?;
        if self.header.partition_entry_lba != 2 {
            self.header.partition_entry_lba = self.header.last_usable_lba + 1;
        }

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

        Ok(())
    }

    pub fn find_free_sectors(&self) -> Vec<(u64, u64)> {
        let mut positions = Vec::new();
        positions.push(self.header.first_usable_lba - 1);
        for partition in self.partitions.iter().filter(|x| x.is_used()) {
            positions.push(partition.starting_lba);
            positions.push(partition.ending_lba);
        }
        positions.push(self.header.last_usable_lba + 1);
        positions.sort();

        positions
            .chunks(2)
            .map(|x| (x[0] + 1, x[1] - x[0] - 1))
            .filter(|(_, l)| *l > 0)
            .collect()
    }

    pub fn find_first_place(&self, size: u64) -> Option<u64> {
        self.find_free_sectors()
            .iter()
            .filter(|(_, l)| *l >= size)
            .next()
            .map(|&(i, _)| i)
    }

    pub fn find_last_place(&self, size: u64) -> Option<u64> {
        self.find_free_sectors()
            .iter()
            .filter(|(_, l)| *l >= size)
            .last()
            .map(|&(i, l)| i + l - size)
    }

    pub fn find_optimal_place(&self, size: u64) -> Option<u64> {
        let mut slots = self
            .find_free_sectors()
            .iter()
            .cloned()
            .filter(|(_, l)| *l >= size)
            .collect::<Vec<_>>();
        slots.sort_by(|(_, l1), (_, l2)| l1.cmp(l2));
        slots.first().map(|&(i, _)| i)
    }

    pub fn get_maximum_partition_size(&self) -> Result<u64> {
        self.find_free_sectors()
            .iter()
            .map(|(_, l)| *l)
            .max()
            .ok_or(Error::NoSpaceLeft)
    }

    pub fn sort(&mut self) {
        self.partitions
            .sort_by(|a, b| a.starting_lba.cmp(&b.starting_lba));
    }

    pub fn remove(&mut self, i: u32) {
        assert!(i != 0, "invalid partition index: 0");
        self.partitions[i as usize - 1] = GPTPartitionEntry::empty();
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &GPTPartitionEntry)> {
        self.partitions
            .iter()
            .enumerate()
            .map(|(i, x)| (i as u32 + 1, x))
    }
}

impl Index<u32> for GPT {
    type Output = GPTPartitionEntry;

    fn index<'a>(&'a self, i: u32) -> &'a GPTPartitionEntry {
        assert!(i != 0, "invalid partition index: 0");
        &self.partitions[i as usize - 1]
    }
}

impl IndexMut<u32> for GPT {
    fn index_mut<'a>(&'a mut self, i: u32) -> &'a mut GPTPartitionEntry {
        assert!(i != 0, "invalid partition index: 0");
        &mut self.partitions[i as usize - 1]
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;

    const DISK1: &'static str = "tests/fixtures/disk1.img";
    const DISK2: &'static str = "tests/fixtures/disk2.img";

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
                gpt.partition_entry_lba * ss + gpt.size_of_partition_entry as u64,
            ))
            .unwrap();
            let bar = GPTPartitionEntry::read_from(&mut f).unwrap();
            assert!(!bar.is_unused());

            let mut unused = 0;
            let mut used = 0;
            let mut partitions = Vec::new();
            for i in 0..gpt.number_of_partition_entries {
                f.seek(SeekFrom::Start(
                    gpt.partition_entry_lba * ss + i as u64 * gpt.size_of_partition_entry as u64,
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
                    gpt.partition_entry_lba * ss + i as u64 * gpt.size_of_partition_entry as u64,
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
        let gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();

        assert_eq!(gpt.find_first_place(10000), None);
        assert_eq!(gpt.find_first_place(4), Some(44));
        assert_eq!(gpt.find_first_place(8), Some(53));
    }

    #[test]
    fn add_partition_right() {
        let gpt = GPT::find_from(&mut fs::File::open(DISK2).unwrap()).unwrap();

        assert_eq!(gpt.find_last_place(10000), None);
        assert_eq!(gpt.find_last_place(5), Some(90));
        assert_eq!(gpt.find_last_place(20), Some(50));
    }

    #[test]
    fn add_partition_optimal() {
        let gpt = GPT::find_from(&mut fs::File::open(DISK2).unwrap()).unwrap();

        assert_eq!(gpt.find_optimal_place(10000), None);
        assert_eq!(gpt.find_optimal_place(5), Some(80));
        assert_eq!(gpt.find_optimal_place(20), Some(16));
    }

    #[test]
    fn sort_partitions() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();

        let starting_lba = gpt.find_first_place(4).unwrap();
        gpt.partitions[10] = GPTPartitionEntry {
            starting_lba,
            ending_lba: starting_lba + 3,
            attribute_bits: 0,
            partition_type_guid: [1; 16],
            partition_name: "Baz".into(),
            unique_parition_guid: [1; 16],
        };

        assert_eq!(
            gpt.partitions
                .iter()
                .filter(|x| x.is_used())
                .map(|x| x.partition_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Foo", "Bar", "Baz"]
        );
        gpt.sort();
        assert_eq!(
            gpt.partitions
                .iter()
                .filter(|x| x.is_used())
                .map(|x| x.partition_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Foo", "Baz", "Bar"]
        );
    }

    #[test]
    fn add_partition_on_unsorted_table() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();

        let starting_lba = gpt.find_first_place(4).unwrap();
        gpt.partitions[10] = GPTPartitionEntry {
            starting_lba,
            ending_lba: starting_lba + 3,
            attribute_bits: 0,
            partition_type_guid: [1; 16],
            partition_name: "Baz".into(),
            unique_parition_guid: [1; 16],
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
            gpt.header.crc32_checksum = 1;
            let backup_lba = gpt.header.backup_lba;
            cur.seek(SeekFrom::Start(ss)).unwrap();
            serialize_into(&mut cur, &gpt.header).unwrap();
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            assert_eq!(gpt.header.backup_lba, 1);
            let partition_entry_lba = gpt.header.partition_entry_lba;
            gpt.write_into(&mut cur).unwrap();
            let mut gpt = GPT::read_from(&mut cur, ss).unwrap();
            assert_eq!(gpt.header.primary_lba, 1);
            assert_eq!(gpt.header.backup_lba, backup_lba);
            assert_eq!(gpt.header.partition_entry_lba, 2);

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

            gpt.remove(1);
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
    fn get_maximum_partition_size_on_empty_disk() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();

        for i in 1..=gpt.header.number_of_partition_entries {
            gpt.remove(i);
        }

        assert_eq!(gpt.get_maximum_partition_size().ok(), Some(33));
    }

    #[test]
    fn get_maximum_partition_size_on_disk_full() {
        let mut gpt = GPT::find_from(&mut fs::File::open(DISK1).unwrap()).unwrap();

        for partition in gpt.partitions.iter_mut().skip(1) {
            partition.partition_type_guid = [0; 16];
        }
        gpt.partitions[0].starting_lba = gpt.header.first_usable_lba;
        gpt.partitions[0].ending_lba = gpt.header.last_usable_lba;;

        assert!(gpt.get_maximum_partition_size().is_err());
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
}
