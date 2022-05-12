use clap::{ArgEnum, Parser};
use std::path::PathBuf;

#[derive(ArgEnum, Clone, Debug)]
pub enum Column {
    Device,
    Start,
    End,
    Sectors,
    Size,
    Type,
    Guid,
    Attributes,
    Name,
}

#[derive(Parser, Debug)]
#[clap(version)]
pub struct Opt {
    /// display partitions and exit
    #[clap(short = 'l', long = "list")]
    pub print: bool,

    /// output columns
    #[clap(
        short = 'o',
        long = "output",
        arg_enum,
        default_value = "device,start,end,sectors,size,type,guid,attributes,name",
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn verify_app() {
        use clap::CommandFactory;
        Opt::command().debug_assert();
    }
}
