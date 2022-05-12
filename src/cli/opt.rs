use clap::{ArgEnum, StructOpt};
use std::path::PathBuf;

#[derive(ArgEnum, Clone, Debug)]
#[clap(rename_all = "verbatim")]
pub enum Column {
    Device,
    Start,
    End,
    Sectors,
    Size,
    Type,
    GUID,
    Attributes,
    Name,
}

#[derive(StructOpt, Debug)]
#[structopt()]
pub struct Opt {
    /// display partitions and exit
    #[structopt(short = 'l', long = "list")]
    pub print: bool,

    /// output columns
    #[structopt(
        short = 'o',
        long = "output",
        arg_enum,
        default_value = "Device,Start,End,Sectors,Size,Type,GUID,Attributes,Name",
        use_delimiter = true
    )]
    pub columns: Vec<Column>,

    /// device to open
    #[structopt(name = "DEVICE", parse(from_os_str))]
    pub device: PathBuf,

    /// initialize a new GPT on the disk
    #[structopt(short = 'i', long = "init")]
    pub init: bool,

    /// sector size
    #[structopt(short = 'b', long = "sector-size")]
    pub sector_size: Option<u64>,

    /// partition alignment
    #[structopt(short = 'a', long = "align")]
    pub align: Option<u64>,
}
