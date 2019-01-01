#[macro_use]
extern crate lazy_static;
extern crate bincode;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate crc;
#[macro_use]
extern crate log;
extern crate env_logger;

mod gpt;
mod table;
mod types;
mod uuid;

use self::gpt::*;
use self::types::PartitionTypeGUID;
use self::uuid::UUID;
use std::fs;
use std::io;
use std::io::{Seek, SeekFrom, Write};

const BYTE_UNITS: &'static [&'static str] = &["kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

macro_rules! format_bytes {
    ($value:expr) => {
        BYTE_UNITS
            .iter()
            .enumerate()
            .map(|(i, u)| ($value / 1000_u64.pow(i as u32 + 1), u))
            .take_while(|(i, _)| *i > 100)
            .map(|(i, u)| format!("{} {}", i, u))
            .last()
            .unwrap_or(format!("{} B", $value))
    };
}

fn ask(prompt: &str) -> io::Result<String> {
    print!("{} ", prompt);
    io::stdout().flush()?;
    let mut answer = String::new();
    match std::io::stdin().read_line(&mut answer) {
        Ok(0) => Err(io::Error::new(io::ErrorKind::Other, "EOF")),
        Err(err) => Err(err),
        Ok(_) => Ok(answer.trim().to_string()),
    }
}

fn execute(full_command: &str) {
    let mut it = full_command.split(' ');
    let command = it.next().unwrap();
    let args = it.collect::<Vec<_>>();
    debug!("command: {:?}", command);
    debug!("command arguments: {:?}", args);

    if command == "p" {
        for path in args {
            match print(path) {
                Ok(()) => {}
                Err(err) => println!("could not open {:?}: {}", path, err),
            }
        }
    } else {
        println!("{}: unknown command", command);
    }
}

fn main() {
    env_logger::init();

    loop {
        match ask("Command (m for help):") {
            Ok(command) => {
                if command == "q" {
                    break;
                } else {
                    debug!("received command: {:?}", command);
                    execute(command.as_str());
                }
            }
            Err(err) => {
                println!("{}", err);
                break;
            }
        }
    }
}

fn print(path: &str) -> Result<(), Error> {
    debug!("opening GPT from: {:?}", path);
    let mut f = fs::File::open(path)?;
    let len = f.seek(SeekFrom::End(0))?;
    let gpt = GPT::find_from(&mut f)?;
    let usable = gpt.header.last_usable_lba - gpt.header.first_usable_lba + 1;

    println!("Sector size: {} bytes", gpt.sector_size);
    println!("Disk size: {} bytes", len);
    println!(
        "Usable sectors: {} ({} bytes)",
        usable,
        usable * gpt.sector_size
    );
    println!("Disk identifier: {}", gpt.header.disk_guid.display_uuid());

    let mut table = table::Table::new(6);
    table.add_cell_rtl("Start");
    table.add_cell_rtl("End");
    table.add_cell_rtl("Sectors");
    table.add_cell_rtl("Size");
    table.add_cell("Type");
    table.add_cell("Name");
    for p in gpt.partitions.iter().filter(|x| x.is_used()) {
        table.add_cell_rtl(&format!("{}", p.starting_lba));
        table.add_cell_rtl(&format!("{}", p.ending_lba));
        table.add_cell_rtl(&format!("{}", p.ending_lba - p.starting_lba));
        table.add_cell_rtl(&format_bytes!(
            (p.ending_lba - p.starting_lba) * gpt.sector_size
        ));
        table.add_cell(&format!(
            "{}",
            p.partition_type_guid.display_partition_type_guid()
        ));
        table.add_cell(&format!("{}", p.partition_name.as_str()));
    }
    println!("{}", table);

    Ok(())
}
