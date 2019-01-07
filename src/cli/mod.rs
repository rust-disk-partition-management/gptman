#[macro_use]
mod macros;
mod add_partition;
mod change_disk_guid;
mod change_partition_guid;
mod change_partition_name;
mod change_type;
mod copy_partition;
mod delete_partition;
pub mod error;
mod fix_partitions_order;
mod opt;
mod print;
mod print_raw_data;
mod resize_partition;
mod table;
mod toggle_attributes;
mod toggle_legacy_bootable;
mod toggle_no_block_io;
mod toggle_required;
mod write;

use self::add_partition::*;
use self::change_disk_guid::*;
use self::change_partition_guid::*;
use self::change_partition_name::*;
use self::change_type::*;
use self::copy_partition::*;
use self::delete_partition::*;
use self::fix_partitions_order::*;
use self::print::*;
use self::print_raw_data::*;
use self::resize_partition::*;
use self::toggle_attributes::*;
use self::toggle_legacy_bootable::*;
use self::toggle_no_block_io::*;
use self::toggle_required::*;
use self::write::*;

use self::error::*;
pub use self::opt::*;
use crate::gpt::GPT;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

const BYTE_UNITS: &'static [&'static str] = &["kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

pub fn format_bytes(value: u64) -> String {
    BYTE_UNITS
        .iter()
        .enumerate()
        .map(|(i, u)| (value / 1000_u64.pow(i as u32 + 1), u))
        .take_while(|(i, _)| *i > 10)
        .map(|(i, u)| format!("{} {}", i, u))
        .last()
        .unwrap_or(format!("{} B ", value))
}

pub fn execute<F>(full_command: &str, opt: &Opt, len: u64, gpt: &mut GPT, ask: &F) -> Result<bool>
where
    F: Fn(&str) -> Result<String>,
{
    let mut it = full_command.split(' ');
    let command = it.next().unwrap();
    let args = it.collect::<Vec<_>>();
    debug!("command: {:?}", command);
    debug!("command arguments: {:?}", args);

    if command == "p" {
        if args.is_empty() {
            print(&opt, &opt.device, gpt, len)?;
        } else {
            for path in args {
                match open_and_print(&opt, &path.into()) {
                    Ok(()) => {}
                    Err(err) => println!("could not open {:?}: {}", path, err),
                }
            }
        }
    } else if command == "n" {
        add_partition(gpt, ask)?;
    } else if command == "d" {
        delete_partition(gpt, ask)?;
    } else if command == "f" {
        fix_partitions_order(gpt);
    } else if command == "w" {
        write(gpt, &opt.device)?;
        return Ok(true);
    } else if command == "t" {
        change_type(gpt, ask)?;
    } else if command == "u" {
        change_partition_guid(gpt, ask)?;
    } else if command == "i" {
        change_disk_guid(gpt, ask)?;
    } else if command == "L" {
        change_partition_name(gpt, ask)?;
    } else if command == "A" {
        toggle_legacy_bootable(gpt, ask)?;
    } else if command == "B" {
        toggle_no_block_io(gpt, ask)?;
    } else if command == "R" {
        toggle_required(gpt, ask)?;
    } else if command == "S" {
        toggle_attributes(gpt, ask)?;
    } else if command == "r" {
        resize_partition(gpt, ask)?;
    } else if command == "c" {
        copy_partition(gpt, &opt.device, ask)?;
    } else if command == "D" {
        print_raw_data(gpt, &opt.device)?;
    } else {
        println!("{}: unknown command", command);
    }

    Ok(false)
}

pub fn open_and_print(opt: &Opt, path: &PathBuf) -> Result<()> {
    debug!("opening GPT from: {:?}", path);
    let mut f = fs::File::open(path)?;
    let len = f.seek(SeekFrom::End(0))?;
    let gpt = GPT::find_from(&mut f)?;

    print(opt, path, &gpt, len)
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
        .ok_or(Error::new("no available slot"))?;

    let i = ask_with_default!(
        ask,
        |x| u32::from_str_radix(x, 10),
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
        .ok_or(Error::new("no partition found"))?;

    let i = loop {
        match ask_with_default!(
            ask,
            |x| u32::from_str_radix(x, 10),
            "Enter used partition number",
            default_i
        )? {
            i if gpt[i].is_unused() => println!("Partition number {} is not used", i),
            i => break i,
        }
    };

    Ok(i)
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
