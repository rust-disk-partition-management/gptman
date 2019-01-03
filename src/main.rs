#[macro_use]
extern crate lazy_static;
extern crate bincode;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate crc;
#[macro_use]
extern crate log;
extern crate clap;
extern crate env_logger;
extern crate linefeed;

mod gpt;
#[macro_use]
mod cli;

use self::cli::error::*;
use self::cli::*;
use self::gpt::GPT;
use clap::{App, Arg};
use linefeed::{Interface, ReadResult, Signal};
use std::fs;
use std::io::{Seek, SeekFrom};

const NAME: &'static str = env!("CARGO_PKG_NAME");
const DESCRIPTION: &'static str = env!("CARGO_PKG_DESCRIPTION");
const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");

fn main() {
    let matches = App::new(NAME)
        .version(VERSION)
        .author(AUTHORS)
        .about(DESCRIPTION)
        .arg(
            Arg::with_name("print")
                .short("p")
                .long("print")
                .help("Print the device's GPT and exit"),
        )
        .arg(
            Arg::with_name("device")
                .required(true)
                .help("Device to open"),
        )
        .get_matches();

    env_logger::init();

    let disk_path = matches.value_of("device").unwrap();

    if matches.is_present("print") {
        main_unwrap!(open_and_print(disk_path));
    }

    let interface = Interface::new("gptman").expect("open terminal interface");
    let ask = |prompt: &str| -> Result<String> {
        main_unwrap!(interface.set_prompt(&format!("{} ", prompt)));
        interface.set_report_signal(Signal::Interrupt, true);
        match main_unwrap!(interface.read_line()) {
            ReadResult::Input(line) => Ok(line),
            ReadResult::Eof => Err("EOF".into()),
            _ => Err("^C".into()),
        }
    };

    let (mut gpt, len) = main_unwrap!(open_disk(disk_path));
    loop {
        match ask("Command (m for help):") {
            Ok(command) => {
                if command == "q" {
                    break;
                } else if command != "" {
                    debug!("received command: {:?}", command);
                    match execute(command.as_str(), disk_path, len, &mut gpt, ask) {
                        Ok(()) => {}
                        Err(err) => println!("{}", err),
                    }
                }
            }
            Err(_) => {
                println!();
                break;
            }
        }
    }
}

fn open_disk(path: &str) -> Result<(GPT, u64)> {
    let mut f = fs::File::open(path)?;
    let gpt = GPT::find_from(&mut f)?;
    let len = f.seek(SeekFrom::End(0))?;

    Ok((gpt, len))
}
