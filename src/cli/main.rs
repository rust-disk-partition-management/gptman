#![allow(clippy::from_str_radix_10, clippy::seek_from_current)]

mod attribute_bits;
mod commands;
mod display_bytes;
mod error;
mod opt;
mod table;
mod types;
mod uuid;

use self::commands::{execute, print};
use self::error::*;
use self::opt::*;
use self::uuid::generate_random_uuid;
use clap::Parser;
#[cfg(target_os = "linux")]
use gptman::linux::get_sector_size;
use gptman::GPT;
use linefeed::{Interface, ReadResult, Signal};
use std::fs;
use std::io::{Seek, SeekFrom};

macro_rules! main_unwrap {
    ($e:expr) => {{
        match $e {
            Ok(x) => x,
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        }
    }};
}

fn main() {
    let opt = Opt::parse();

    if opt.print {
        let (mut gpt, len) = main_unwrap!(open_disk(&opt));

        if let Some(align) = opt.align {
            gpt.align = align;
        }

        main_unwrap!(print(&opt, &opt.device, &gpt, len, false));
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

    let (mut gpt, len) = main_unwrap!(if opt.init {
        new_gpt(&opt, &ask)
    } else {
        open_disk(&opt)
    });

    if let Some(align) = opt.align {
        gpt.align = align;
    }

    loop {
        match ask("Command (m for help):") {
            Ok(command) => {
                if command == "q" {
                    break;
                } else if !command.is_empty() {
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

fn open_disk(opt: &Opt) -> Result<(GPT, u64)> {
    let mut f = fs::File::open(&opt.device)?;
    let gpt = if let Some(ss) = opt.sector_size {
        GPT::read_from(&mut f, ss)?
    } else {
        GPT::find_from(&mut f)?
    };
    let len = f.seek(SeekFrom::End(0))?;

    Ok((gpt, len))
}

fn new_gpt<F>(opt: &Opt, ask: &F) -> Result<(GPT, u64)>
where
    F: Fn(&str) -> Result<String>,
{
    println!("Initializing a new GPT on {}...", opt.device.display());

    let mut f = fs::File::open(&opt.device)?;
    let len = f.seek(SeekFrom::End(0))?;

    #[allow(unused_mut)]
    let mut sector_size = opt.sector_size.unwrap_or(512);

    #[cfg(target_os = "linux")]
    {
        match get_sector_size(&mut f) {
            Err(err) => println!("failed to get sector size of device: {}", err),
            Ok(x) => sector_size = x,
        }
    }

    println!("Sector size: {} bytes", sector_size);

    if GPT::find_from(&mut f).is_ok() {
        println!("WARNING: a GPT already exists on the device");
    }

    ask("Do you wish to continue (yes/no)?").and_then(|x| {
        if x == "yes" {
            Ok(())
        } else if x == "no" {
            Err(Error::new("Aborted."))
        } else {
            Err(Error::new(&format!(
                "Invalid answer '{}'. Please type 'yes' or 'no'.",
                x
            )))
        }
    })?;

    let guid = generate_random_uuid();
    let gpt = GPT::new_from(&mut f, sector_size, guid)?;
    println!("GPT created.");

    Ok((gpt, len))
}
