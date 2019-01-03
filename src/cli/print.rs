use self::attribute_bits::*;
use self::table::*;
use self::types::*;
use self::uuid::*;
use crate::cli::*;
use crate::gpt::GPT;

pub fn print(path: &str, gpt: &GPT, len: u64) -> Result<()> {
    let usable = gpt.header.last_usable_lba - gpt.header.first_usable_lba + 1;

    println!("Sector size: {} bytes", gpt.sector_size);
    println!("Disk size: {} ({} bytes)", format_bytes(len), len);
    println!(
        "Usable sectors: {}-{} ({} sectors)",
        gpt.header.first_usable_lba, gpt.header.last_usable_lba, usable,
    );
    println!(
        "Usable space: {} ({} bytes)",
        format_bytes(usable * gpt.sector_size),
        usable * gpt.sector_size
    );
    println!("Disk identifier: {}", gpt.header.disk_guid.display_uuid());
    println!();

    let mut table = Table::new(9);
    table.add_cell("Device");
    table.add_cell_rtl("Start");
    table.add_cell_rtl("End");
    table.add_cell_rtl("Sectors");
    table.add_cell_rtl("Size");
    table.add_cell("Type");
    table.add_cell("GUID");
    table.add_cell("Attributes");
    table.add_cell("Name");
    for (i, p) in gpt
        .partitions
        .iter()
        .enumerate()
        .filter(|(_, x)| x.is_used())
    {
        table.add_cell(&format!("{}{}", path, i + 1));
        table.add_cell_rtl(&format!("{}", p.starting_lba));
        table.add_cell_rtl(&format!("{}", p.ending_lba));
        table.add_cell_rtl(&format!("{}", p.ending_lba - p.starting_lba + 1));
        table.add_cell_rtl(&format_bytes(
            (p.ending_lba - p.starting_lba) * gpt.sector_size,
        ));
        table.add_cell(&format!(
            "{}",
            p.partition_type_guid.display_partition_type_guid()
        ));
        table.add_cell(&format!("{}", p.unique_parition_guid.display_uuid()));
        table.add_cell(&format!(
            "{}",
            p.attribute_bits
                .display_attribute_bits(p.partition_type_guid)
        ));
        table.add_cell(&format!("{}", p.partition_name.as_str()));
    }
    print!("{}", table);

    Ok(())
}
