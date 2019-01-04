use crate::cli::*;
use crate::uuid::{convert_str_to_array, generate_random_uuid};

pub fn change_disk_guid<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let default_disk_guid = generate_random_uuid();

    let disk_guid = match ask("Disk GUID (default: random):")?.as_ref() {
        "" => default_disk_guid,
        x => convert_str_to_array(x)?,
    };

    gpt.header.disk_guid = disk_guid;

    Ok(())
}
