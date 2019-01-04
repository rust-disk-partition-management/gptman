use crate::cli::*;
use crate::uuid::{convert_str_to_array, generate_random_uuid};

pub fn change_partition_guid<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let default_i = gpt
        .iter()
        .filter(|(_, x)| x.is_used())
        .map(|(i, _)| i)
        .last()
        .ok_or(Error::new("no partition found"))?;
    let default_unique_parition_guid = generate_random_uuid();

    let i = ask_with_default!(
        ask,
        |x| u32::from_str_radix(x, 10),
        "Partition number",
        default_i
    )?;

    let unique_parition_guid = match ask("Partition GUID (default: random):")?.as_ref() {
        "" => default_unique_parition_guid,
        x => convert_str_to_array(x)?,
    };

    gpt[i].unique_parition_guid = unique_parition_guid;

    Ok(())
}
