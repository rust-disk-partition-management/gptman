#[macro_use]
mod macros;
mod add_partition;
mod change_disk_guid;
mod change_partition_guid;
mod change_partition_name;
mod change_type;
mod delete_partition;
pub mod error;
mod fix_partitions_order;
mod opt;
mod print;
mod table;
mod toggle_legacy_bootable;
mod toggle_no_block_io;
mod write;

use self::add_partition::*;
use self::change_disk_guid::*;
use self::change_partition_guid::*;
use self::change_partition_name::*;
use self::change_type::*;
use self::delete_partition::*;
use self::fix_partitions_order::*;
use self::print::*;
use self::toggle_legacy_bootable::*;
use self::toggle_no_block_io::*;
use self::write::*;

use self::error::*;
pub use self::opt::*;
use crate::gpt::GPT;
use std::fs;
use std::io::{Seek, SeekFrom};
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
