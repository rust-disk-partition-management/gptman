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
extern crate prettytable;

mod gpt;
mod types;

use self::gpt::*;
use prettytable::format::FormatBuilder;
use prettytable::{Attr, Cell, Row, Table};
use std::fs;
use std::io;
use std::io::{Seek, SeekFrom, Write};

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

trait UUID {
    fn uuid(&self) -> String;
}

impl UUID for [u8; 16] {
    fn uuid(&self) -> String {
        let mut digits: Vec<_> = self.iter().collect();
        let mut uuid: Vec<String> = Vec::new();
        uuid.extend(digits.drain(..4).rev().map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..2).rev().map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..2).rev().map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..2).map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..).map(|x| format!("{:02X}", x)));

        uuid.into_iter().collect()
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
    println!("Disk identifier: {}", gpt.header.disk_guid.uuid());

    let mut table = Table::new();
    let format = FormatBuilder::new().column_separator(' ').build();
    table.set_format(format);
    table.add_row(Row::new(vec![
        Cell::new("start").with_style(Attr::Bold),
        Cell::new("end").with_style(Attr::Bold),
        Cell::new("sectors").with_style(Attr::Bold),
        Cell::new("name").with_style(Attr::Bold),
    ]));
    for p in gpt.partitions.iter().filter(|x| x.is_used()) {
        table.add_row(Row::new(vec![
            Cell::new(&format!("{}", p.starting_lba)),
            Cell::new(&format!("{}", p.ending_lba)),
            Cell::new(&format!("{}", p.ending_lba - p.starting_lba)),
            Cell::new(p.partition_name.as_str()),
        ]));
    }
    table.printstd();

    Ok(())
}
