#[macro_use]
mod macros;
mod add_partition;
mod delete_partition;
pub mod error;
mod fix_partitions_order;
mod print;
mod table;
mod write;

use self::add_partition::*;
use self::delete_partition::*;
use self::fix_partitions_order::*;
use self::print::*;
use self::write::*;

use self::error::*;
use crate::gpt::GPT;
use std::fs;
use std::io::{Seek, SeekFrom};

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

pub fn execute<F>(full_command: &str, disk: &str, len: u64, gpt: &mut GPT, ask: F) -> Result<bool>
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
            print(disk, gpt, len)?;
        } else {
            for path in args {
                match open_and_print(path) {
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
        write(gpt, disk)?;
        return Ok(true);
    } else {
        println!("{}: unknown command", command);
    }

    Ok(false)
}

pub fn open_and_print(path: &str) -> Result<()> {
    debug!("opening GPT from: {:?}", path);
    let mut f = fs::File::open(path)?;
    let len = f.seek(SeekFrom::End(0))?;
    let gpt = GPT::find_from(&mut f)?;

    print(path, &gpt, len)
}
