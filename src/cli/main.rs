#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;
extern crate env_logger;
extern crate linefeed;
#[macro_use]
extern crate nix;
extern crate gptman;
extern crate rand;
extern crate structopt;

mod attribute_bits;
mod commands;
mod error;
mod opt;
mod protective_mbr;
mod table;
mod types;
mod uuid;

use self::commands::{execute, print};
use self::error::*;
use self::opt::*;
use self::uuid::generate_random_uuid;
use gptman::GPT;
use linefeed::{Interface, ReadResult, Signal};
use std::fs;
use std::io::{Seek, SeekFrom};
use std::os::linux::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use structopt::StructOpt;

ioctl_read_bad!(blksszget, 0x1268, u64);

const S_IFMT: u32 = 0o170_000;
const S_IFBLK: u32 = 0o60_000;

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
    let opt = Opt::from_args();

    env_logger::init();

    if opt.print {
        let (mut gpt, len) = main_unwrap!(open_disk(&opt));

        if let Some(align) = opt.align {
            gpt.align = align;
        }

        main_unwrap!(print(&opt, &opt.device, &gpt, len));
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
    let metadata = fs::metadata(&opt.device).expect("could not get metadata");

    let mut sector_size = opt.sector_size.unwrap_or(512);

    if opt.sector_size.is_none() && metadata.st_mode() & S_IFMT == S_IFBLK {
        println!("getting sector size from device");
        match unsafe { blksszget(f.as_raw_fd(), &mut sector_size) } {
            Err(err) => println!("ioctl call failed: {}", err),
            Ok(0) => {}
            Ok(x) => println!("ioctl returned error code: {}", x),
        }
    }

    println!("Sector size: {} bytes", sector_size);

    if GPT::find_from(&mut f).is_ok() {
        println!("WARNING: a GPT already exists on the device");
    }

    ask("Do you wish to continue (yes/no)?").and_then(|x| {
        if x == "yes" {
            Ok(())
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
