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
mod protective_mbr;
mod types;
mod uuid;

use self::cli::error::*;
use self::cli::*;
use self::gpt::GPT;
use self::uuid::generate_random_uuid;
use linefeed::{Interface, ReadResult, Signal};
use std::fs;
use std::io::{Seek, SeekFrom};
use std::os::linux::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use structopt::StructOpt;

ioctl_read_bad!(blksszget, 0x1268, u64);

const S_IFMT: u32 = 0o00170000;
const S_IFBLK: u32 = 0o0060000;

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

    let (mut gpt, len) = main_unwrap!(if opt.init {
        new_gpt(&opt.device, &ask)
    } else {
        open_disk(&opt.device)
    });

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

fn new_gpt<F>(path: &PathBuf, ask: &F) -> Result<(GPT, u64)>
where
    F: Fn(&str) -> Result<String>,
{
    println!("Initializing a new GPT on {}...", path.display());

    let mut f = fs::File::open(&path)?;
    let len = f.seek(SeekFrom::End(0))?;
    let metadata = fs::metadata(&path).expect("could not get metadata");

    let mut sector_size = 512;

    if metadata.st_mode() & S_IFMT == S_IFBLK {
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
