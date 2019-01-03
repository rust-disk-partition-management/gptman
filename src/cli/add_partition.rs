use crate::cli::*;
use crate::gpt::{GPTPartitionEntry, GPT};

pub fn add_partition<F>(gpt: &mut GPT, ask: F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let default_i = gpt
        .iter()
        .filter(|(_, x)| x.is_unused())
        .map(|(i, _)| i)
        .next()
        .ok_or(Error::new("no available slot"))?;
    let max_size: u64 = gpt.get_maximum_partition_size()?;

    let i = ask_with_default!(
        ask,
        |x| u32::from_str_radix(x, 10),
        "Partition number",
        default_i
    )?;
    let size = ask_with_default!(
        ask,
        |x| u64::from_str_radix(x, 10),
        "Partition size",
        max_size
    )?;
    if size == 0 {
        return Err("The size must be at least 1 sector".into());
    }

    let optimal_lba = gpt
        .find_optimal_place(size)
        .ok_or(Error::new("not enough space on device"))?;
    let starting_lba = ask_with_default!(
        ask,
        |x| u64::from_str_radix(x, 10),
        "Partition starting LBA",
        optimal_lba
    )?;

    gpt[i] = GPTPartitionEntry {
        starting_lba,
        ending_lba: starting_lba + size - 1,
        attribute_bits: 0,
        partition_name: "".into(),
        partition_type_guid: [1; 16],
        unique_parition_guid: [1; 16],
    };

    Ok(())
}
