use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Debug, ValueEnum)]
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
#[command(next_display_order = None, version)]
pub struct Opt {
    /// display partitions and exit
    #[arg(short = 'l', long = "list")]
    pub print: bool,

    /// output columns
    #[arg(
        short = 'o',
        long = "output",
        value_enum,
        default_value = "device,start,end,sectors,size,type,guid,attributes,name",
        value_delimiter = ','
    )]
    pub columns: Vec<Column>,

    /// device to open
    #[arg(value_name = "DEVICE")]
    pub device: PathBuf,

    /// initialize a new GPT on the disk
    #[arg(short = 'i', long = "init")]
    pub init: bool,

    /// sector size
    #[arg(short = 'b', long = "sector-size")]
    pub sector_size: Option<u64>,

    /// partition alignment
    #[arg(short = 'a', long = "align")]
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
