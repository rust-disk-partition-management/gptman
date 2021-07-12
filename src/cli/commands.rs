use crate::attribute_bits::AttributeBits;
use crate::error::*;
use crate::opt::Opt;
use crate::table::Table;
use crate::types::PartitionTypeGUID;
use crate::uuid::{convert_str_to_array, generate_random_uuid, UUID};
#[cfg(target_os = "linux")]
use gptman::linux::reread_partition_table;
use gptman::{GPTPartitionEntry, GPT};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{PathBuf, Path};

const BYTE_UNITS: &[&str] = &["kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

fn format_bytes(value: u64) -> String {
    BYTE_UNITS
        .iter()
        .enumerate()
        .map(|(i, u)| (value / 1000_u64.pow(i as u32 + 1), u))
        .take_while(|(i, _)| *i > 10)
        .map(|(i, u)| format!("{} {}", i, u))
        .last()
        .unwrap_or_else(|| format!("{} B ", value))
}

macro_rules! ask_with_default {
    ($ask:expr, $parser:expr, $prompt:expr, $default:expr) => {
        loop {
            let input = $ask(&format!("{} (default {}):", $prompt, $default))?;

            if input == "" {
                break Ok($default);
            } else {
                match $parser(&input) {
                    Err(err) => {
                        println!("{}", err);
                    }
                    x => break x,
                }
            }
        }
    };
}

pub fn execute<F>(full_command: &str, opt: &Opt, len: u64, gpt: &mut GPT, ask: &F) -> Result<bool>
where
    F: Fn(&str) -> Result<String>,
{
    let mut it = full_command.split(' ');
    let command = it.next().unwrap();
    let args = it.collect::<Vec<_>>();

    match command {
        "m" => help(),
        "p" | "P" => {
            let disk_order = command == "P";

            if args.is_empty() {
                print(&opt, &opt.device, gpt, len, disk_order)?;
            } else {
                for path in args {
                    match open_and_print(&opt, &path.as_ref(), disk_order) {
                        Ok(()) => {}
                        Err(err) => println!("could not open {:?}: {}", path, err),
                    }
                }
            }
        }
        "n" => add_partition(gpt, ask)?,
        "d" => delete_partition(gpt, ask)?,
        "f" => fix_partitions_order(gpt),
        "w" => {
            write(gpt, &opt)?;
            return Ok(true);
        }
        "t" => change_type(gpt, ask)?,
        "u" => change_partition_guid(gpt, ask)?,
        "i" => change_disk_guid(gpt, ask)?,
        "L" => change_partition_name(gpt, ask)?,
        "A" => toggle_legacy_bootable(gpt, ask)?,
        "B" => toggle_no_block_io(gpt, ask)?,
        "R" => toggle_required(gpt, ask)?,
        "S" => toggle_attributes(gpt, ask)?,
        "r" => resize_partition(gpt, ask)?,
        "c" => copy_partition(gpt, &opt.device, ask)?,
        "D" => print_raw_data(gpt, &opt.device)?,
        "a" => change_alignment(gpt, ask)?,
        "Z" => randomize(gpt),
        "s" => swap_partition_index(gpt, ask)?,
        "C" => copy_all_partitions(gpt, &opt.device, ask)?,
        x => println!("{}: unknown command", x),
    }

    Ok(false)
}

fn help() {
    println!("\nHelp:\n");
    println!("  a   change partition alignment");
    println!("  A   toggle the legacy BIOS bootable flag");
    println!("  B   toggle the no block IO protocol flag");
    println!("  c   copy a partition from another device (or the same)");
    println!("  C   copy all partitions from another device (or the same)");
    println!("  d   delete a partition");
    println!("  D   print the raw data of the disklabel from the device");
    println!("  f   fix partitions order");
    println!("  i   change disk GUID");
    println!("  L   change partition name");
    println!("  n   add a new partition");
    println!("  p   print the partition table (in order of the array)");
    println!("  P   print the partition table (in order of the disk)");
    println!("  r   resize a partition");
    println!("  R   toggle the required partition flag");
    println!("  s   swap partition indexes");
    println!("  S   toggle the GUID specific bits");
    println!("  t   change a partition type");
    println!("  u   change partition UUID");
    println!("  Z   randomize disk GUID and all partition's GUID");
    println!();
    println!("  q   exit without saving");
    println!("  w   write table to disk and exit");
    println!();
}

fn open_and_print(opt: &Opt, path: &Path, disk_order: bool) -> Result<()> {
    let mut f = fs::File::open(path)?;
    let len = f.seek(SeekFrom::End(0))?;
    let gpt = GPT::find_from(&mut f)?;

    print(opt, path, &gpt, len, disk_order)
}

fn ask_free_slot<F>(gpt: &GPT, ask: &F) -> Result<u32>
where
    F: Fn(&str) -> Result<String>,
{
    let default_i = gpt
        .iter()
        .filter(|(_, x)| x.is_unused())
        .map(|(i, _)| i)
        .next()
        .ok_or("no available slot")?;

    let i = ask_with_default!(
        ask,
        |x| match u32::from_str_radix(x, 10) {
            Ok(i) => {
                if i <= 128 && i > 0 {
                    Ok(i)
                } else {
                    Err("the partition index must be between 1 and 128")
                }
            }
            Err(_) => Err("could not parse integer"),
        },
        "Enter free partition number",
        default_i
    )?;
    if gpt[i].is_used() {
        println!("WARNING: partition {} is going to be overwritten", i);
    }

    Ok(i)
}

fn ask_used_slot<F>(gpt: &GPT, ask: &F) -> Result<u32>
where
    F: Fn(&str) -> Result<String>,
{
    let default_i = gpt
        .iter()
        .filter(|(_, x)| x.is_used())
        .map(|(i, _)| i)
        .last()
        .ok_or("no partition found")?;

    let i = loop {
        match ask_with_default!(
            ask,
            |x| match u32::from_str_radix(x, 10) {
                Ok(i) => {
                    if i <= 128 && i > 0 {
                        Ok(i)
                    } else {
                        Err("the partition index must be between 1 and 128")
                    }
                }
                Err(_) => Err("could not parse integer"),
            },
            "Enter used partition number",
            default_i
        )? {
            i if gpt[i].is_unused() => println!("Partition number {} is not used", i),
            i => break i,
        }
    };

    Ok(i)
}

fn ask_partition<F>(ask: &F, prompt: &str) -> Result<u32>
where
    F: Fn(&str) -> Result<String>,
{
    Ok(loop {
        match u32::from_str_radix(ask(prompt)?.as_ref(), 10) {
            Err(err) => println!("{}", err),
            Ok(i) if i > 128 || i == 0 => println!("the partition index must be between 1 and 128"),
            Ok(i) => break i,
        }
    })
}

fn ask_starting_lba<F>(gpt: &GPT, ask: &F, size: u64) -> Result<u64>
where
    F: Fn(&str) -> Result<String>,
{
    let optimal_lba = gpt
        .find_optimal_place(size)
        .ok_or("not enough space on device")?;
    let first_lba = gpt.find_first_place(size).unwrap();
    let last_lba = gpt.find_last_place(size).unwrap();

    let starting_lba = ask_with_default!(
        ask,
        |x| match x {
            ">" => Ok(last_lba),
            "<" => Ok(first_lba),
            "^" => Ok(optimal_lba),
            x => parse_lba(gpt, x, first_lba, last_lba),
        },
        &format!(
            "Partition starting LBA (< {}, > {}, sectors, kib, mib, kb, mb, ...)",
            first_lba, last_lba
        ),
        optimal_lba
    )?;

    Ok(starting_lba)
}

fn parse_lba(gpt: &GPT, value: &str, min: u64, max: u64) -> Result<u64> {
    let n = value.trim_end_matches(char::is_alphabetic).parse::<u64>()?;
    let unit = (*value)
        .to_uppercase()
        .as_str()
        .trim_start_matches(char::is_numeric)
        .to_string();
    let result = match unit.as_str() {
        "KIB" => (n * 1024 - 1) / gpt.sector_size + 1,
        "MIB" => (n * 1024_u64.pow(2) - 1) / gpt.sector_size + 1,
        "GIB" => (n * 1024_u64.pow(3) - 1) / gpt.sector_size + 1,
        "TIB" => (n * 1024_u64.pow(4) - 1) / gpt.sector_size + 1,
        "PIB" => (n * 1024_u64.pow(5) - 1) / gpt.sector_size + 1,
        "EIB" => (n * 1024_u64.pow(6) - 1) / gpt.sector_size + 1,
        "ZIB" => (n * 1024_u64.pow(7) - 1) / gpt.sector_size + 1,
        "YIB" => (n * 1024_u64.pow(8) - 1) / gpt.sector_size + 1,
        "KB" => (n * 1000 - 1) / gpt.sector_size + 1,
        "MB" => (n * 1000_u64.pow(2) - 1) / gpt.sector_size + 1,
        "GB" => (n * 1000_u64.pow(3) - 1) / gpt.sector_size + 1,
        "TB" => (n * 1000_u64.pow(4) - 1) / gpt.sector_size + 1,
        "PB" => (n * 1000_u64.pow(5) - 1) / gpt.sector_size + 1,
        "EB" => (n * 1000_u64.pow(6) - 1) / gpt.sector_size + 1,
        "ZB" => (n * 1000_u64.pow(7) - 1) / gpt.sector_size + 1,
        "YB" => (n * 1000_u64.pow(8) - 1) / gpt.sector_size + 1,
        "" => value.parse::<u64>()?,
        x => return Err(Error::new(&format!("Invalid unit: {}", x))),
    };
    let aligned_up = ((result - 1) / gpt.align + 1) * gpt.align;

    if aligned_up < min {
        Err("The value is too small".into())
    } else if aligned_up > max {
        Err("The value is too big".into())
    } else {
        Ok(aligned_up)
    }
}

#[test]
fn test_parse_lba() {
    use std::io;

    let ss = 2;
    let data = vec![0; 2 * 2048 * 10];
    let mut cur = io::Cursor::new(data);
    let mut gpt = GPT::new_from(&mut cur, ss, [0; 16]).unwrap();

    gpt.align = 1;

    assert_eq!(parse_lba(&gpt, "1", 1, 1), Ok(1));
    assert_eq!(parse_lba(&gpt, "42", 1, 42), Ok(42));
    assert!(parse_lba(&gpt, "42", 1, 1).is_err());
    assert!(parse_lba(&gpt, "1", 2, 1).is_err());
    assert!(parse_lba(&gpt, "1dl", 1, u64::max_value()).is_err());
    assert_eq!(parse_lba(&gpt, "1kib", 1, u64::max_value()), Ok(1024 / ss));
    assert_eq!(
        parse_lba(&gpt, "5kib", 1, u64::max_value()),
        Ok(5 * 1024 / ss)
    );
    assert_eq!(
        parse_lba(&gpt, "1mib", 1, u64::max_value()),
        Ok(1024 * 1024 / ss)
    );
    assert_eq!(
        parse_lba(&gpt, "1gib", 1, u64::max_value()),
        Ok(1024_u64.pow(3) / ss)
    );
    assert_eq!(parse_lba(&gpt, "1kb", 1, u64::max_value()), Ok(1000 / ss));
    assert_eq!(
        parse_lba(&gpt, "1mb", 1, u64::max_value()),
        Ok(1_000_000 / ss)
    );
    assert_eq!(
        parse_lba(&gpt, "1gb", 1, u64::max_value()),
        Ok(1_000_000_000 / ss)
    );

    gpt.align = 5;

    assert_eq!(parse_lba(&gpt, "1", 1, u64::max_value()), Ok(5));
    assert_eq!(parse_lba(&gpt, "42", 1, u64::max_value()), Ok(45));
    assert_eq!(
        parse_lba(&gpt, "1kib", 1, u64::max_value()),
        Ok(1024 / ss / 5 * 5 + 5)
    );
    assert_eq!(
        parse_lba(&gpt, "4kib", 1, u64::max_value()),
        Ok(4 * 1024 / ss / 5 * 5 + 5)
    );
    assert_eq!(
        parse_lba(&gpt, "5kib", 1, u64::max_value()),
        Ok(5 * 1024 / ss)
    );

    gpt.sector_size = 4096;
    gpt.align = 1;

    assert_eq!(parse_lba(&gpt, "1kib", 1, u64::max_value()), Ok(1));
}

fn print_bytes<R>(reader: &mut R, limit: usize) -> Result<()>
where
    R: Read + Seek,
{
    let mut bytes_read = 0;
    let mut pos = reader.seek(SeekFrom::Current(0))?;
    let mut skipping = false;

    while bytes_read < limit {
        let mut data = vec![0; 16.min(limit - bytes_read)];
        let len = reader.read(&mut data)?;
        pos += len as u64;
        bytes_read += len;

        if data == [0; 16] {
            if !skipping {
                skipping = true;
                println!("*");
            }
            continue;
        } else {
            skipping = false;
        }

        print!("{:08x}  ", pos);
        let mut it = data.iter().take(len);
        for b in it.by_ref().take(8) {
            print!("{:02x} ", b);
        }
        for b in it.by_ref() {
            print!(" {:02x}", b);
        }
        println!();
    }

    Ok(())
}

pub fn print(opt: &Opt, path: &Path, gpt: &GPT, len: u64, disk_order: bool) -> Result<()> {
    use crate::opt::Column;

    let usable = gpt.header.last_usable_lba - gpt.header.first_usable_lba + 1;

    println!("Sector size: {} bytes", gpt.sector_size);
    println!(
        "Partition alignment: {} ({} bytes)",
        gpt.align,
        gpt.align * gpt.sector_size
    );
    println!("Disk size: {} ({} bytes)", format_bytes(len), len);
    println!(
        "Usable sectors: {}-{} ({} sectors)",
        gpt.header.first_usable_lba, gpt.header.last_usable_lba, usable,
    );
    println!(
        "Free sectors: {}",
        gpt.find_free_sectors()
            .iter()
            .map(|(i, l)| format!(
                "{}-{} ({})",
                i,
                i + l - 1,
                format_bytes(l * gpt.sector_size).trim()
            ))
            .collect::<Vec<_>>()
            .join(", "),
    );
    println!(
        "Usable space: {} ({} bytes)",
        format_bytes(usable * gpt.sector_size),
        usable * gpt.sector_size,
    );
    println!("Disk identifier: {}", gpt.header.disk_guid.display_uuid());
    println!();

    let misaligned = gpt
        .iter()
        .filter(|(_, x)| x.is_used() && x.starting_lba % gpt.align != 0)
        .map(|(i, _)| format!("{}", i))
        .collect::<Vec<_>>();
    if !misaligned.is_empty() {
        println!(
            "WARNING: some partitions are not aligned: {}\n",
            misaligned.join(", ")
        );
    }

    let mut table = Table::new(opt.columns.len());
    for column in opt.columns.iter() {
        match column {
            Column::Device => table.add_cell("Device"),
            Column::Start => table.add_cell_rtl("Start"),
            Column::End => table.add_cell_rtl("End"),
            Column::Sectors => table.add_cell_rtl("Sectors"),
            Column::Size => table.add_cell_rtl("Size"),
            Column::Type => table.add_cell("Type"),
            Column::GUID => table.add_cell("GUID"),
            Column::Attributes => table.add_cell("Attributes"),
            Column::Name => table.add_cell("Name"),
        }
    }
    let mut base_path = path.display().to_string();
    if base_path.ends_with(char::is_numeric) {
        base_path += "p";
    }

    let mut partitions: Vec<_> = gpt.iter().filter(|(_, x)| x.is_used()).collect();

    if disk_order {
        partitions.sort_by_key(|(_, x)| x.starting_lba);
    }

    for (i, p) in partitions {
        for column in opt.columns.iter() {
            match column {
                Column::Device => table.add_cell(&format!("{}{}", base_path, i)),
                Column::Start => table.add_cell_rtl(&format!("{}", p.starting_lba)),
                Column::End => table.add_cell_rtl(&format!("{}", p.ending_lba)),
                Column::Sectors => table.add_cell_rtl(&format!("{}", p.size()?)),
                Column::Size => table.add_cell_rtl(&format_bytes(p.size()? * gpt.sector_size)),
                Column::Type => {
                    table.add_cell(p.partition_type_guid.display_partition_type_guid().as_str())
                }
                Column::GUID => table.add_cell(p.unique_partition_guid.display_uuid().as_str()),
                Column::Attributes => table.add_cell(
                    p.attribute_bits
                        .display_attribute_bits(p.partition_type_guid)
                        .as_str(),
                ),
                Column::Name => table.add_cell(p.partition_name.as_str()),
            }
        }
    }
    print!("{}", table);

    Ok(())
}

fn add_partition<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let max_size: u64 = gpt.get_maximum_partition_size()?;
    let default_unique_partition_guid = generate_random_uuid();

    let i = ask_free_slot(gpt, ask)?;

    let size = ask_with_default!(
        ask,
        |x| parse_lba(gpt, x, 1, max_size),
        "Partition size (sectors, kib, mib, kb, mb, ...)",
        max_size
    )?;
    if size == 0 {
        return Err("The size must be at least 1 sector".into());
    }

    let partition_type_guid = ask_partition_type_guid(ask)?;
    let starting_lba = ask_starting_lba(gpt, ask, size)?;
    let partition_name = ask("Partition name:")?.as_str().into();

    let unique_partition_guid = match ask("Partition GUID (default: random):")?.as_ref() {
        "" => default_unique_partition_guid,
        x => convert_str_to_array(x)?,
    };

    gpt[i] = GPTPartitionEntry {
        starting_lba,
        ending_lba: starting_lba + size - 1,
        attribute_bits: 0,
        partition_name,
        partition_type_guid,
        unique_partition_guid,
    };

    Ok(())
}

fn fix_partitions_order(gpt: &mut GPT) {
    gpt.sort();
}

fn write(gpt: &mut GPT, opt: &Opt) -> Result<()> {
    let mut f = fs::OpenOptions::new().write(true).open(&opt.device)?;
    gpt.write_into(&mut f)?;

    if opt.init {
        GPT::write_protective_mbr_into(&mut f, gpt.sector_size)?;
        println!("protective MBR has been written");
    }

    #[cfg(target_os = "linux")]
    {
        if let Err(err) = reread_partition_table(&mut f) {
            println!("rereading partition table failed: {}", err);
        }
    }

    Ok(())
}

fn change_type<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    gpt[i].partition_type_guid = ask_partition_type_guid(ask)?;

    Ok(())
}

fn ask_partition_type_guid<F>(ask: &F) -> Result<[u8; 16]>
where
    F: Fn(&str) -> Result<String>,
{
    use crate::types::TYPE_MAP;

    let mut categories: Vec<_> = TYPE_MAP.keys().collect();
    categories.sort_by_key(|&a| a.to_ascii_lowercase());

    loop {
        match ask("Partition type GUID (type L to list all types):")?.as_ref() {
            "" => {}
            "q" => break,
            "L" => loop {
                println!("Category:");
                for (i, cat) in categories.iter().enumerate() {
                    println!("{:2} => {}", i + 1, cat);
                }

                match ask("Choose category (q to go back):")?.as_ref() {
                    "" => {}
                    "q" => break,
                    i => loop {
                        if let Some(types_map) = i.parse::<usize>()
                            .ok()
                            .and_then(|x| categories.get(x - 1))
                            .and_then(|x| TYPE_MAP.get(*x))
                        {
                            let mut types: Vec<_> = types_map.iter().collect();
                            types.sort_by(|a, b| a.1.cmp(b.1));
                            let types: Vec<(usize, &(&[u8; 16], &&str))> =
                                types.iter().enumerate().collect();

                            println!("Partition types:");
                            for (i, (guid, name)) in types.iter() {
                                println!("{:2} => {}: {}", i + 1, guid.display_uuid(), name);
                            }

                            match ask("Choose partition type (q to go back):")?.as_ref() {
                                "" => {}
                                "q" => break,
                                i => {
                                    if let Some(arr) = i.parse::<usize>()
                                        .ok()
                                        .and_then(|x| types.get(x - 1).map(|(_, (arr, _))| **arr))
                                    {
                                        return Ok(arr);
                                    }
                                }
                            }
                        }
                    },
                }
            },
            x => match convert_str_to_array(x) {
                Ok(arr) => return Ok(arr),
                Err(err) => {
                    println!("{}", err);
                }
            },
        }
    }

    Err(Error::new("aborted."))
}

fn change_partition_guid<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let default_unique_partition_guid = generate_random_uuid();

    let i = ask_used_slot(gpt, ask)?;

    let unique_partition_guid = match ask("Partition GUID (default: random):")?.as_ref() {
        "" => default_unique_partition_guid,
        x => convert_str_to_array(x)?,
    };

    gpt[i].unique_partition_guid = unique_partition_guid;

    Ok(())
}

fn change_disk_guid<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let default_disk_guid = generate_random_uuid();

    let disk_guid = match ask("Disk GUID (default: random):")?.as_ref() {
        "" => default_disk_guid,
        x => convert_str_to_array(x)?,
    };

    gpt.header.disk_guid = disk_guid;

    Ok(())
}

fn change_partition_name<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    let partition_name = ask("Partition name:")?.as_str().into();

    gpt[i].partition_name = partition_name;

    Ok(())
}

fn toggle_legacy_bootable<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    gpt[i].attribute_bits ^= 0b100;

    Ok(())
}

fn toggle_no_block_io<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    gpt[i].attribute_bits ^= 0b10;

    Ok(())
}

fn toggle_required<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    gpt[i].attribute_bits ^= 0b1;

    Ok(())
}

fn toggle_attributes<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    let attributes = loop {
        match ask("Enter GUID specific bits (48-63):")?.as_str() {
            "" => return Ok(()),
            s => {
                let attributes = s
                    .split(',')
                    .map(|x| u64::from_str_radix(x, 10))
                    .collect::<Vec<_>>();

                if let Some(attr) = attributes.iter().find(|x| x.is_err()) {
                    println!("{}", attr.as_ref().unwrap_err());
                } else if let Some(attr) = attributes
                    .iter()
                    .map(|x| x.as_ref().unwrap())
                    .find(|x| **x < 48 || **x > 63)
                {
                    println!("invalid attribute: {}", attr);
                } else {
                    #[allow(clippy::redundant_closure)]
                    break attributes.into_iter().map(|x| x.unwrap());
                }
            }
        }
    };

    for x in attributes {
        gpt[i].attribute_bits ^= 1 << x;
    }

    Ok(())
}

fn resize_partition<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    let free_sectors = gpt.find_free_sectors();

    let max_size = {
        let align = gpt.align;
        let p = &mut gpt[i];

        (p.size()?
            + free_sectors
                .iter()
                .skip_while(|(m, _)| *m < p.starting_lba)
                .take(1)
                .find(|(m, _)| *m == ((p.ending_lba + 1) / align + 1) * align)
                .map(|(_, n)| n)
                .unwrap_or(&0))
            / align
            * align
    };

    let size = ask_with_default!(
        ask,
        |x| parse_lba(gpt, x, 1, max_size),
        "Partition size",
        max_size
    )?;

    gpt[i].ending_lba = gpt[i].starting_lba + size - 1;

    Ok(())
}

fn copy_partition<F>(dst_gpt: &mut GPT, dst_path: &Path, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let src_path: PathBuf =
        match ask(&format!("From disk (default {}):", dst_path.display()))?.as_str() {
            "" => dst_path.to_path_buf(),
            x => x.into(),
        };
    let src_gpt = GPT::find_from(&mut fs::File::open(src_path)?)?;

    let src_i = ask_used_slot(&src_gpt, ask)?;
    let dst_i = ask_free_slot(dst_gpt, ask)?;

    let size_in_bytes = src_gpt[src_i].size()? * src_gpt.sector_size;
    if size_in_bytes % dst_gpt.sector_size != 0 {
        return Err(Error::new(&format!(
            "Partition size {} is not aligned to sector size {}",
            size_in_bytes, dst_gpt.sector_size
        )));
    }
    let size = size_in_bytes / dst_gpt.sector_size;

    let starting_lba = ask_starting_lba(dst_gpt, ask, size)?;

    dst_gpt[dst_i] = src_gpt[src_i].clone();
    dst_gpt[dst_i].starting_lba = starting_lba;
    dst_gpt[dst_i].ending_lba = starting_lba + size - 1;

    Ok(())
}

fn print_raw_data(gpt: &GPT, path: &Path) -> Result<()> {
    let mut f = fs::File::open(path)?;

    print_table(&mut f, "First sector", 0, gpt.sector_size as u32)?;
    print_table(
        &mut f,
        "GPT header",
        gpt.header.primary_lba * gpt.sector_size,
        gpt.header.header_size,
    )?;
    print_table(
        &mut f,
        "GPT entries",
        gpt.header.partition_entry_lba * gpt.sector_size,
        gpt.header.number_of_partition_entries * gpt.header.size_of_partition_entry,
    )?;

    Ok(())
}

fn print_table<R>(reader: &mut R, label: &str, offset: u64, size: u32) -> Result<()>
where
    R: Read + Seek,
{
    println!("{}: offset = {}, size = {}", label, offset, size);
    reader.seek(SeekFrom::Start(offset))?;
    print_bytes(reader, size as usize)?;
    println!();

    Ok(())
}

fn change_alignment<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    gpt.align = ask_with_default!(
        ask,
        |x| u64::from_str_radix(x, 10),
        "Partition alignment",
        gpt.align
    )?;

    Ok(())
}

fn randomize(gpt: &mut GPT) {
    gpt.header.disk_guid = generate_random_uuid();

    for (_, p) in gpt.iter_mut() {
        p.unique_partition_guid = generate_random_uuid();
    }
}

fn swap_partition_index<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i1 = ask_partition(ask, "Enter first partition number:")?;
    let i2 = ask_partition(ask, "Enter second partition number:")?;

    let p1 = gpt[i1].clone();
    gpt[i1] = gpt[i2].clone();
    gpt[i2] = p1;

    Ok(())
}

fn copy_all_partitions<F>(dst_gpt: &mut GPT, dst_path: &Path, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let src_path: PathBuf =
        match ask(&format!("From disk (default {}):", dst_path.display()))?.as_str() {
            "" => dst_path.to_path_buf(),
            x => x.into(),
        };
    let src_gpt = GPT::find_from(&mut fs::File::open(src_path)?)?;

    for (src_i, p) in src_gpt.iter().filter(|(_, x)| x.is_used()) {
        let size_in_bytes = p.size()? * src_gpt.sector_size;
        if size_in_bytes % dst_gpt.sector_size != 0 {
            return Err(Error::new(&format!(
                "Partition size {} is not aligned to sector size {}",
                size_in_bytes, dst_gpt.sector_size
            )));
        }
        let size = size_in_bytes / dst_gpt.sector_size;

        println!(
            "Copy partition {} of {} sectors ({}):",
            src_i,
            size,
            format_bytes(size_in_bytes)
        );
        let dst_i = ask_free_slot(dst_gpt, ask)?;
        let starting_lba = ask_starting_lba(dst_gpt, ask, size)?;

        dst_gpt[dst_i] = p.clone();
        dst_gpt[dst_i].starting_lba = starting_lba;
        dst_gpt[dst_i].ending_lba = starting_lba + size - 1;
    }

    Ok(())
}

fn delete_partition<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    gpt.remove(i)?;

    Ok(())
}
