use crate::cli::*;
use crate::gpt::{GPTPartitionEntry, GPT};
use crate::uuid::{convert_str_to_array, generate_random_uuid};

pub fn add_partition<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let max_size: u64 = gpt.get_maximum_partition_size()?;
    let default_unique_parition_guid = generate_random_uuid();

    let i = ask_free_slot(gpt, ask)?;

    let size = ask_with_default!(
        ask,
        |x| u64::from_str_radix(x, 10),
        "Partition size",
        max_size
    )?;
    if size == 0 {
        return Err("The size must be at least 1 sector".into());
    }

    let partition_type_guid = ask_partition_type_guid(ask)?;

    let optimal_lba = gpt
        .find_optimal_place(size)
        .ok_or(Error::new("not enough space on device"))?;
    let first_lba = gpt.find_first_place(size).unwrap();
    let last_lba = gpt.find_last_place(size).unwrap();
    let starting_lba = ask_with_default!(
        ask,
        |x| match x {
            ">" => Ok(last_lba),
            "<" => Ok(first_lba),
            "^" => Ok(optimal_lba),
            x => u64::from_str_radix(x, 10),
        },
        &format!("Partition starting LBA (< {}, > {})", first_lba, last_lba),
        optimal_lba
    )?;

    let partition_name = ask("Partition name:")?.as_str().into();

    let unique_parition_guid = match ask("Partition GUID (default: random):")?.as_ref() {
        "" => default_unique_parition_guid,
        x => convert_str_to_array(x)?,
    };

    gpt[i] = GPTPartitionEntry {
        starting_lba,
        ending_lba: starting_lba + size - 1,
        attribute_bits: 0,
        partition_name,
        partition_type_guid,
        unique_parition_guid,
    };

    Ok(())
}
