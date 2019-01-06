#[macro_use]
extern crate lazy_static;
extern crate bincode;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate crc;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;
extern crate env_logger;
extern crate linefeed;
#[macro_use]
extern crate nix;
extern crate rand;
extern crate structopt;

mod gpt;
#[macro_use]
mod cli;
mod attribute_bits;
mod types;
mod uuid;

use self::cli::error::*;
use self::cli::*;
use self::gpt::GPT;
use linefeed::{Interface, ReadResult, Signal};
use std::fs;
use std::io::{Seek, SeekFrom};
use std::path::PathBuf;
use structopt::StructOpt;

fn main() {
    let opt = Opt::from_args();

    env_logger::init();

    if opt.print {
        main_unwrap!(open_and_print(&opt, &opt.device));
        return;
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

    let (mut gpt, len) = main_unwrap!(open_disk(&opt.device));
    loop {
        match ask("Command (m for help):") {
            Ok(command) => {
                if command == "q" {
                    break;
                } else if command != "" {
                    debug!("received command: {:?}", command);
                    match execute(command.as_str(), &opt, len, &mut gpt, &ask) {
                        Ok(false) => {}
                        Ok(true) => break,
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

fn open_disk(path: &PathBuf) -> Result<(GPT, u64)> {
    let mut f = fs::File::open(path)?;
    let gpt = GPT::find_from(&mut f)?;
    let len = f.seek(SeekFrom::End(0))?;

    Ok((gpt, len))
}
