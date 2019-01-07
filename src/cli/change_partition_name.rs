use crate::cli::*;
use crate::gpt::GPT;

pub fn change_partition_name<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    let partition_name = ask("Partition name:")?.as_str().into();

    gpt[i].partition_name = partition_name;

    Ok(())
}
