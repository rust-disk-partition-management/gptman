use crate::cli::*;
use crate::gpt::GPT;

pub fn delete_partition<F>(gpt: &mut GPT, ask: F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let default_i = gpt
        .partitions
        .iter()
        .enumerate()
        .filter(|(_, x)| x.is_used())
        .map(|(i, _)| i + 1)
        .last()
        .ok_or(Error::new("no partition found"))?;
    let i = ask_with_default!(
        ask,
        |x| usize::from_str_radix(x, 10),
        "Partition number",
        default_i
    )?;

    gpt.partitions[i - 1].partition_type_guid = [0; 16];

    Ok(())
}
