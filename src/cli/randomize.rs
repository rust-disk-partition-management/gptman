use crate::cli::*;
use crate::uuid::generate_random_uuid;

pub fn randomize(gpt: &mut GPT) -> Result<()> {
    gpt.header.disk_guid = generate_random_uuid();

    for (_, p) in gpt.iter_mut() {
        p.unique_parition_guid = generate_random_uuid();
    }

    Ok(())
}
