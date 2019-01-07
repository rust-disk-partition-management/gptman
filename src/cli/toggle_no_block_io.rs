use crate::cli::*;
use crate::gpt::GPT;

pub fn toggle_no_block_io<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    gpt[i].attribute_bits ^= 0b10;

    Ok(())
}
