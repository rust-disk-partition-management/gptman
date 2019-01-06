use crate::cli::*;
use crate::gpt::GPT;

pub fn change_partition_name<F>(gpt: &mut GPT, ask: F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let default_i = gpt
        .iter()
        .filter(|(_, x)| x.is_used())
        .map(|(i, _)| i)
        .last()
        .ok_or(Error::new("no partition found"))?;
    let i = ask_with_default!(
        ask,
        |x| u32::from_str_radix(x, 10),
        "Partition number",
        default_i
    )?;

    let partition_name = ask("Partition name:")?.as_str().into();

    gpt[i].partition_name = partition_name;

    Ok(())
}
