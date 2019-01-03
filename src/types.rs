use crate::uuid::UUID;
use std::collections::HashMap;

lazy_static! {
    pub static ref TYPE_MAP: HashMap<&'static str, HashMap<[u8; 16], &'static str>> = {
        fn to_array(uuid: &str) -> [u8; 16] {
            let mut arr = [0; 16];
            let mut digits: Vec<_> = uuid
                .chars()
                .filter(|&x| x != '-')
                .collect::<Vec<_>>()
                .chunks(2)
                .map(|x| x.iter().collect::<String>())
                .map(|x| u8::from_str_radix(x.as_str(), 16).unwrap())
                .collect();
            let mut reordered = Vec::new();
            reordered.extend(digits.drain(..4).rev());
            reordered.extend(digits.drain(..2).rev());
            reordered.extend(digits.drain(..2).rev());
            reordered.extend(digits.drain(..2));
            reordered.extend(digits.drain(..));
            for (e, v) in arr.iter_mut().zip(reordered.iter()) {
                *e = *v;
            }

            arr
        }

        let mut cat = HashMap::new();

        let mut m = HashMap::new();
        m.insert(
            to_array("00000000-0000-0000-0000-000000000000"),
            "Unused entry",
        );
        m.insert(
            to_array("024DEE41-33E7-11D3-9D69-0008C781F39F"),
            "MBR partition scheme",
        );
        m.insert(
            to_array("C12A7328-F81F-11D2-BA4B-00A0C93EC93B"),
            "EFI System partition",
        );
        m.insert(
            to_array("21686148-6449-6E6F-744E-656564454649"),
            "BIOS boot partition",
        );
        m.insert(
            to_array("D3BFE2DE-3DAF-11DF-BA40-E3A556D89593"),
            "Intel Fast Flash (iFFS) partition (for Intel Rapid Start technology)",
        );
        m.insert(
            to_array("F4019732-066E-4E12-8273-346C5641494F"),
            "Sony boot partition",
        );
        m.insert(
            to_array("BFBFAFE7-A34F-448A-9A5B-6213EB736C22"),
            "Lenovo boot partition",
        );
        cat.insert("N/A", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("E3C9E316-0B5C-4DB8-817D-F92DF00215AE"),
            "Microsoft Reserved Partition (MSR)",
        );
        m.insert(
            to_array("EBD0A0A2-B9E5-4433-87C0-68B6B72699C7"),
            "Basic data partition",
        );
        m.insert(
            to_array("5808C8AA-7E8F-42E0-85D2-E1E90434CFB3"),
            "Logical Disk Manager (LDM) metadata partition",
        );
        m.insert(
            to_array("AF9B60A0-1431-4F62-BC68-3311714A69AD"),
            "Logical Disk Manager data partition",
        );
        m.insert(
            to_array("DE94BBA4-06D1-4D40-A16A-BFD50179D6AC"),
            "Windows Recovery Environment",
        );
        m.insert(
            to_array("37AFFC90-EF7D-4E96-91C3-2D7AE055B174"),
            "IBM General Parallel File System (GPFS) partition",
        );
        m.insert(
            to_array("E75CAF8F-F680-4CEE-AFA3-B001E56EFC2D"),
            "Storage Spaces partition",
        );
        cat.insert("Windows", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("75894C1E-3AEB-11D3-B7C1-7B03A0000000"),
            "Data partition",
        );
        m.insert(
            to_array("E2A1E728-32E3-11D6-A682-7B03A0000000"),
            "Service Partition",
        );
        cat.insert("HP-UX", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("0FC63DAF-8483-4772-8E79-3D69D8477DE4"),
            "Linux filesystem data",
        );
        m.insert(
            to_array("A19D880F-05FC-4D3B-A006-743F0F84911E"),
            "RAID partition",
        );
        m.insert(
            to_array("44479540-F297-41B2-9AF7-D131D5F0458A"),
            "Root partition (x86)",
        );
        m.insert(
            to_array("4F68BCE3-E8CD-4DB1-96E7-FBCAF984B709"),
            "Root partition (x86-64)",
        );
        m.insert(
            to_array("69DAD710-2CE4-4E3C-B16C-21A1D49ABED3"),
            "Root partition (32-bit ARM)",
        );
        m.insert(
            to_array("B921B045-1DF0-41C3-AF44-4C6F280D3FAE"),
            "Root partition (64-bit ARM/AArch64)",
        );
        m.insert(
            to_array("A2A0D0EB-E5B9-3344-87C0-68B6B72699C7"),
            "Data partition",
        );
        m.insert(
            to_array("AF3DC60F-8384-7247-8E79-3D69D8477DE4"),
            "Data partition",
        );
        m.insert(
            to_array("0657FD6D-A4AB-43C4-84E5-0933C84B4F4F"),
            "Swap partition",
        );
        m.insert(
            to_array("E6D6D379-F507-44C2-A23C-238F2A3DF928"),
            "Logical Volume Manager (LVM) partition",
        );
        m.insert(
            to_array("933AC7E1-2EB4-4F13-B844-0E14E2AEF915"),
            "/home partition",
        );
        m.insert(
            to_array("3B8F8425-20E0-4F3B-907F-1A25A76F98E8"),
            "/srv (server data) partition",
        );
        m.insert(
            to_array("7FFEC5C9-2D00-49B7-8941-3EA10A5586B7"),
            "Plain dm-crypt partition",
        );
        m.insert(
            to_array("CA7D7CCB-63ED-4C53-861C-1742536059CC"),
            "LUKS partition",
        );
        m.insert(to_array("8DA63339-0007-60C0-C436-083AC8230908"), "Reserved");
        cat.insert("Linux", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("83BD6B9D-7F41-11DC-BE0B-001560B84F0F"),
            "Boot partition",
        );
        m.insert(
            to_array("516E7CB4-6ECF-11D6-8FF8-00022D09712B"),
            "Data partition",
        );
        m.insert(
            to_array("516E7CB5-6ECF-11D6-8FF8-00022D09712B"),
            "Swap partition",
        );
        m.insert(
            to_array("516E7CB6-6ECF-11D6-8FF8-00022D09712B"),
            "Unix File System (UFS) partition",
        );
        m.insert(
            to_array("516E7CB8-6ECF-11D6-8FF8-00022D09712B"),
            "Vinum volume manager partition",
        );
        m.insert(
            to_array("516E7CBA-6ECF-11D6-8FF8-00022D09712B"),
            "ZFS partition",
        );
        cat.insert("FreeBSD", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("48465300-0000-11AA-AA11-00306543ECAC"),
            "Hierarchical File System Plus (HFS+) partition",
        );
        m.insert(
            to_array("7C3457EF-0000-11AA-AA11-00306543ECAC"),
            "Apple APFS",
        );
        m.insert(
            to_array("55465300-0000-11AA-AA11-00306543ECAC"),
            "Apple UFS container",
        );
        m.insert(to_array("6A898CC3-1DD2-11B2-99A6-080020736631"), "ZFS");
        m.insert(
            to_array("52414944-0000-11AA-AA11-00306543ECAC"),
            "Apple RAID partition",
        );
        m.insert(
            to_array("52414944-5F4F-11AA-AA11-00306543ECAC"),
            "Apple RAID partition, offline",
        );
        m.insert(
            to_array("426F6F74-0000-11AA-AA11-00306543ECAC"),
            "Apple Boot partition (Recovery HD)",
        );
        m.insert(
            to_array("4C616265-6C00-11AA-AA11-00306543ECAC"),
            "Apple Label",
        );
        m.insert(
            to_array("5265636F-7665-11AA-AA11-00306543ECAC"),
            "Apple TV Recovery partition",
        );
        m.insert(
            to_array("53746F72-6167-11AA-AA11-00306543ECAC"),
            "Apple Core Storage (i.e. Lion FileVault) partition",
        );
        m.insert(
            to_array("B6FA30DA-92D2-4A9A-96F1-871EC6486200"),
            "SoftRAID_Status",
        );
        m.insert(
            to_array("2E313465-19B9-463F-8126-8A7993773801"),
            "SoftRAID_Scratch",
        );
        m.insert(
            to_array("FA709C7E-65B1-4593-BFD5-E71D61DE9B02"),
            "SoftRAID_Volume",
        );
        m.insert(
            to_array("BBBA6DF5-F46F-4A89-8F59-8765B2727503"),
            "SoftRAID_Cache",
        );
        cat.insert("macOS / Darwin", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("6A82CB45-1DD2-11B2-99A6-080020736631"),
            "Boot partition",
        );
        m.insert(
            to_array("6A85CF4D-1DD2-11B2-99A6-080020736631"),
            "Root partition",
        );
        m.insert(
            to_array("6A87C46F-1DD2-11B2-99A6-080020736631"),
            "Swap partition",
        );
        m.insert(
            to_array("6A8B642B-1DD2-11B2-99A6-080020736631"),
            "Backup partition",
        );
        m.insert(
            to_array("6A898CC3-1DD2-11B2-99A6-080020736631"),
            "/usr partition",
        );
        m.insert(
            to_array("6A8EF2E9-1DD2-11B2-99A6-080020736631"),
            "/var partition",
        );
        m.insert(
            to_array("6A90BA39-1DD2-11B2-99A6-080020736631"),
            "/home partition",
        );
        m.insert(
            to_array("6A9283A5-1DD2-11B2-99A6-080020736631"),
            "Alternate sector",
        );
        m.insert(
            to_array("6A945A3B-1DD2-11B2-99A6-080020736631"),
            "Reserved partition",
        );
        m.insert(
            to_array("6A9630D1-1DD2-11B2-99A6-080020736631"),
            "Reserved partition",
        );
        m.insert(
            to_array("6A980767-1DD2-11B2-99A6-080020736631"),
            "Reserved partition",
        );
        m.insert(
            to_array("6A96237F-1DD2-11B2-99A6-080020736631"),
            "Reserved partition",
        );
        m.insert(
            to_array("6A8D2AC7-1DD2-11B2-99A6-080020736631"),
            "Reserved partition",
        );
        cat.insert("Solaris / illumos", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("49F48D32-B10E-11DC-B99B-0019D1879648"),
            "Swap partition",
        );
        m.insert(
            to_array("49F48D5A-B10E-11DC-B99B-0019D1879648"),
            "FFS partition",
        );
        m.insert(
            to_array("49F48D82-B10E-11DC-B99B-0019D1879648"),
            "LFS partition",
        );
        m.insert(
            to_array("49F48DAA-B10E-11DC-B99B-0019D1879648"),
            "RAID partition",
        );
        m.insert(
            to_array("2DB519C4-B10F-11DC-B99B-0019D1879648"),
            "Concatenated partition",
        );
        m.insert(
            to_array("2DB519EC-B10F-11DC-B99B-0019D1879648"),
            "Encrypted partition",
        );
        cat.insert("NetBSD", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("FE3A2A5D-4F32-41A7-B725-ACCC3285A309"),
            "Chrome OS kernel",
        );
        m.insert(
            to_array("3CB8E202-3B7E-47DD-8A3C-7FF2A13CFCEC"),
            "Chrome OS rootfs",
        );
        m.insert(
            to_array("2E0A753D-9E48-43B0-8337-B15192CB1B5E"),
            "Chrome OS future use",
        );
        cat.insert("Chrome OS", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("5DFBF5F4-2848-4BAC-AA5E-0D9A20B745A6"),
            "/usr partition (coreos-usr)",
        );
        m.insert(
            to_array("3884DD41-8582-4404-B9A8-E9B84F2DF50E"),
            "Resizable rootfs (coreos-resize)",
        );
        m.insert(
            to_array("C95DC21A-DF0E-4340-8D7B-26CBFA9A03E0"),
            "OEM customizations (coreos-reserved)",
        );
        m.insert(
            to_array("BE9067B9-EA49-4F15-B4F6-F36F8C9E1818"),
            "Root filesystem on RAID (coreos-root-raid)",
        );
        cat.insert("Container Linux by CoreOS", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("42465331-3BA3-10F1-802A-4861696B7521"),
            "Haiku BFS",
        );
        cat.insert("Haiku", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("85D5E45E-237C-11E1-B4B3-E89A8F7FC3A7"),
            "Boot partition",
        );
        m.insert(
            to_array("85D5E45A-237C-11E1-B4B3-E89A8F7FC3A7"),
            "Data partition",
        );
        m.insert(
            to_array("85D5E45B-237C-11E1-B4B3-E89A8F7FC3A7"),
            "Swap partition",
        );
        m.insert(
            to_array("0394EF8B-237E-11E1-B4B3-E89A8F7FC3A7"),
            "Unix File System (UFS) partition",
        );
        m.insert(
            to_array("85D5E45C-237C-11E1-B4B3-E89A8F7FC3A7"),
            "Vinum volume manager partition",
        );
        m.insert(
            to_array("85D5E45D-237C-11E1-B4B3-E89A8F7FC3A7"),
            "ZFS partition",
        );
        cat.insert("MidnightBSD", m);

        let mut m = HashMap::new();
        m.insert(to_array("45B0969E-9B03-4F30-B4C6-B4B80CEFF106"), "Journal");
        m.insert(
            to_array("45B0969E-9B03-4F30-B4C6-5EC00CEFF106"),
            "dm-crypt journal",
        );
        m.insert(to_array("4FBD7E29-9D25-41B8-AFD0-062C0CEFF05D"), "OSD");
        m.insert(
            to_array("4FBD7E29-9D25-41B8-AFD0-5EC00CEFF05D"),
            "dm-crypt OSD",
        );
        m.insert(
            to_array("89C57F98-2FE5-4DC0-89C1-F3AD0CEFF2BE"),
            "Disk in creation",
        );
        m.insert(
            to_array("89C57F98-2FE5-4DC0-89C1-5EC00CEFF2BE"),
            "dm-crypt disk in creation",
        );
        m.insert(to_array("CAFECAFE-9B03-4F30-B4C6-B4B80CEFF106"), "Block");
        m.insert(to_array("30CD0809-C2B2-499C-8879-2D6B78529876"), "Block DB");
        m.insert(
            to_array("5CE17FCE-4087-4169-B7FF-056CC58473F9"),
            "Block write-ahead log",
        );
        m.insert(
            to_array("FB3AABF9-D25F-47CC-BF5E-721D1816496B"),
            "Lockbox for dm-crypt keys",
        );
        m.insert(
            to_array("4FBD7E29-8AE0-4982-BF9D-5A8D867AF560"),
            "Multipath OSD",
        );
        m.insert(
            to_array("45B0969E-8AE0-4982-BF9D-5A8D867AF560"),
            "Multipath journal",
        );
        m.insert(
            to_array("CAFECAFE-8AE0-4982-BF9D-5A8D867AF560"),
            "Multipath block",
        );
        m.insert(
            to_array("7F4A666A-16F3-47A2-8445-152EF4D03F6C"),
            "Multipath block",
        );
        m.insert(
            to_array("EC6D6385-E346-45DC-BE91-DA2A7C8B3261"),
            "Multipath block DB",
        );
        m.insert(
            to_array("01B41E1B-002A-453C-9F17-88793989FF8F"),
            "Multipath block write-ahead log",
        );
        m.insert(
            to_array("CAFECAFE-9B03-4F30-B4C6-5EC00CEFF106"),
            "dm-crypt block",
        );
        m.insert(
            to_array("93B0052D-02D9-4D8A-A43B-33A3EE4DFBC3"),
            "dm-crypt block DB",
        );
        m.insert(
            to_array("306E8683-4FE2-4330-B7C0-00A917C16966"),
            "dm-crypt block write-ahead log",
        );
        m.insert(
            to_array("45B0969E-9B03-4F30-B4C6-35865CEFF106"),
            "dm-crypt LUKS journal",
        );
        m.insert(
            to_array("CAFECAFE-9B03-4F30-B4C6-35865CEFF106"),
            "dm-crypt LUKS block",
        );
        m.insert(
            to_array("166418DA-C469-4022-ADF4-B30AFD37F176"),
            "dm-crypt LUKS block DB",
        );
        m.insert(
            to_array("86A32090-3647-40B9-BBBD-38D8C573AA86"),
            "dm-crypt LUKS block write-ahead log",
        );
        m.insert(
            to_array("4FBD7E29-9D25-41B8-AFD0-35865CEFF05D"),
            "dm-crypt LUKS OSD",
        );
        cat.insert("Ceph", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("824CC7A0-36A8-11E3-890A-952519AD3F61"),
            "Data partition",
        );
        cat.insert("OpenBSD", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("CEF5A9AD-73BC-4601-89F3-CDEEEEE321A1"),
            "Power-safe (QNX6) file system",
        );
        cat.insert("QNX", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("C91818F9-8025-47AF-89D2-F030D7000C2C"),
            "Plan 9 partition",
        );
        cat.insert("Plan 9", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("9D275380-40AD-11DB-BF97-000C2911D1B8"),
            "vmkcore (coredump partition)",
        );
        m.insert(
            to_array("AA31E02A-400F-11DB-9590-000C2911D1B8"),
            "VMFS filesystem partition",
        );
        m.insert(
            to_array("9198EFFC-31C0-11DB-8F78-000C2911D1B8"),
            "VMware Reserved",
        );
        cat.insert("VMware ESX", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("2568845D-2332-4675-BC39-8FA5A4748D15"),
            "Bootloader",
        );
        m.insert(
            to_array("114EAFFE-1552-4022-B26E-9B053604CF84"),
            "Bootloader2",
        );
        m.insert(to_array("49A4D17F-93A3-45C1-A0DE-F50B2EBE2599"), "Boot");
        m.insert(to_array("4177C722-9E92-4AAB-8644-43502BFD5506"), "Recovery");
        m.insert(to_array("EF32A33B-A409-486C-9141-9FFB711F6266"), "Misc");
        m.insert(to_array("20AC26BE-20B7-11E3-84C5-6CFDB94711E9"), "Metadata");
        m.insert(to_array("38F428E6-D326-425D-9140-6E0EA133647C"), "System");
        m.insert(to_array("A893EF21-E428-470A-9E55-0668FD91A2D9"), "Cache");
        m.insert(to_array("DC76DDA9-5AC1-491C-AF42-A82591580C0D"), "Data");
        m.insert(
            to_array("EBC597D0-2053-4B15-8B64-E0AAC75F4DB1"),
            "Persistent",
        );
        m.insert(to_array("C5A0AEEC-13EA-11E5-A1B1-001E67CA0C3C"), "Vendor");
        m.insert(to_array("BD59408B-4514-490D-BF12-9878D963F378"), "Config");
        m.insert(to_array("8F68CC74-C5E5-48DA-BE91-A0C8C15E9C80"), "Factory");
        m.insert(
            to_array("9FDAA6EF-4B3F-40D2-BA8D-BFF16BFB887B"),
            "Factory (alt)",
        );
        m.insert(
            to_array("767941D0-2085-11E3-AD3B-6CFDB94711E9"),
            "Fastboot / Tertiary",
        );
        m.insert(to_array("AC6D7924-EB71-4DF8-B48D-E267B27148FF"), "OEM");
        cat.insert("Android-IA", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("19A710A2-B3CA-11E4-B026-10604B889DCF"),
            "Android Meta",
        );
        m.insert(
            to_array("193D1EA4-B3CA-11E4-B075-10604B889DCF"),
            "Android EXT",
        );
        cat.insert("Android 6.0+ ARM", m);

        let mut m = HashMap::new();
        m.insert(to_array("7412F7D5-A156-4B13-81DC-867174929325"), "Boot");
        m.insert(to_array("D4E6E2CD-4469-46F3-B5CB-1BFF57AFC149"), "Config");
        cat.insert("Open Network Install Environment (ONIE)", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("9E1A2D38-C612-4316-AA26-8B49521E5A8B"),
            "PReP boot",
        );
        cat.insert("PowerPC", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("BC13C2FF-59E6-4262-A352-B275FD6F7172"),
            "Shared boot loader configuration",
        );
        cat.insert("freedesktop.org OSes (Linux, etc.)", m);

        let mut m = HashMap::new();
        m.insert(
            to_array("734E5AFE-F61A-11E6-BC64-92361F002671"),
            "Basic data partition (GEM, BGM, F32)",
        );
        cat.insert("Atari TOS", m);

        cat
    };
}

pub trait PartitionTypeGUID {
    fn display_partition_type_guid(&self) -> String;
}

impl PartitionTypeGUID for [u8; 16] {
    fn display_partition_type_guid(&self) -> String {
        TYPE_MAP
            .iter()
            .filter_map(|(cat, m)| m.get(self).map(|x| format!("{} / {}", cat, x)))
            .next()
            .unwrap_or_else(|| self.display_uuid())
    }
}
