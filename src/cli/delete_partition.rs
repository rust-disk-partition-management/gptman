use crate::cli::*;
use crate::gpt::GPT;

pub fn delete_partition<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    gpt.remove(i);

    Ok(())
}
