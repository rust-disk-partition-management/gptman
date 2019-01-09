use crate::cli::*;
use crate::gpt::GPT;

pub fn swap_partition_index<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i1 = ask_partition(ask, "Enter first partition number:")?;
    let i2 = ask_partition(ask, "Enter second partition number:")?;

    let p1 = gpt[i1].clone();
    gpt[i1] = gpt[i2].clone();
    gpt[i2] = p1;

    Ok(())
}
