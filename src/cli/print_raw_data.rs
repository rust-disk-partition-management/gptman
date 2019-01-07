use crate::cli::*;
use crate::gpt::GPT;
use std::path::PathBuf;

pub fn print_raw_data(gpt: &GPT, path: &PathBuf) -> Result<()> {
    let mut f = fs::File::open(path)?;

    print_table(&mut f, "First sector", 0, gpt.sector_size as u32)?;
    print_table(
        &mut f,
        "GPT header",
        gpt.header.primary_lba * gpt.sector_size,
        gpt.header.header_size,
    )?;
    print_table(
        &mut f,
        "GPT entries",
        gpt.header.partition_entry_lba * gpt.sector_size,
        gpt.header.number_of_partition_entries * gpt.header.size_of_partition_entry,
    );

    Ok(())
}

fn print_table<R>(reader: &mut R, label: &str, offset: u64, size: u32) -> Result<()>
where
    R: Read + Seek,
{
    println!("{}: offset = {}, size = {}", label, offset, size);
    reader.seek(SeekFrom::Start(offset))?;
    print_bytes(reader, size as usize)?;
    println!();

    Ok(())
}
