use crate::cli::*;
use crate::uuid::{convert_str_to_array, generate_random_uuid};

pub fn change_partition_guid<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let default_unique_parition_guid = generate_random_uuid();

    let i = ask_used_slot(gpt, ask)?;

    let unique_parition_guid = match ask("Partition GUID (default: random):")?.as_ref() {
        "" => default_unique_parition_guid,
        x => convert_str_to_array(x)?,
    };

    gpt[i].unique_parition_guid = unique_parition_guid;

    Ok(())
}
