use crate::cli::*;
use crate::gpt::GPT;
use std::fs::OpenOptions;

pub fn write(gpt: &mut GPT, path: &str) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true);
    let mut f = options.open(path)?;
    gpt.write_into(&mut f)?;

    Ok(())
}
