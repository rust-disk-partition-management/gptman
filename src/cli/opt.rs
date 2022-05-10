use clap::{ArgEnum, Parser};
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

#[derive(Parser, Debug)]
#[clap()]
pub struct Opt {
    /// display partitions and exit
    #[clap(short = 'l', long = "list")]
    pub print: bool,

    /// output columns
    #[clap(
        short = 'o',
        long = "output",
        arg_enum,
        default_value = "Device,Start,End,Sectors,Size,Type,GUID,Attributes,Name",
        use_value_delimiter = true
    )]
    pub columns: Vec<Column>,

    /// device to open
    #[clap(name = "DEVICE", parse(from_os_str))]
    pub device: PathBuf,

    /// initialize a new GPT on the disk
    #[clap(short = 'i', long = "init")]
    pub init: bool,

    /// sector size
    #[clap(short = 'b', long = "sector-size")]
    pub sector_size: Option<u64>,

    /// partition alignment
    #[clap(short = 'a', long = "align")]
    pub align: Option<u64>,
}
