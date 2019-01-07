use self::table::Table;
use crate::attribute_bits::AttributeBits;
use crate::cli::*;
use crate::gpt::GPT;
use crate::types::PartitionTypeGUID;
use crate::uuid::UUID;
use std::path::PathBuf;

pub fn print(opt: &Opt, path: &PathBuf, gpt: &GPT, len: u64) -> Result<()> {
    let usable = gpt.header.last_usable_lba - gpt.header.first_usable_lba + 1;

    println!("Sector size: {} bytes", gpt.sector_size);
    println!("Disk size: {} ({} bytes)", format_bytes(len), len);
    println!(
        "Usable sectors: {}-{} ({} sectors)",
        gpt.header.first_usable_lba, gpt.header.last_usable_lba, usable,
    );
    println!(
        "Free sectors: {}",
        gpt.find_free_sectors()
            .iter()
            .map(|(i, l)| format!(
                "{}-{} ({})",
                i,
                i + l - 1,
                format_bytes(l * gpt.sector_size).trim()
            ))
            .collect::<Vec<_>>()
            .join(", "),
    );
    println!(
        "Usable space: {} ({} bytes)",
        format_bytes(usable * gpt.sector_size),
        usable * gpt.sector_size,
    );
    println!("Disk identifier: {}", gpt.header.disk_guid.display_uuid());
    println!();

    let mut table = Table::new(opt.columns.len());
    for column in opt.columns.iter() {
        match column {
            Column::Device => table.add_cell("Device"),
            Column::Start => table.add_cell_rtl("Start"),
            Column::End => table.add_cell_rtl("End"),
            Column::Sectors => table.add_cell_rtl("Sectors"),
            Column::Size => table.add_cell_rtl("Size"),
            Column::Type => table.add_cell("Type"),
            Column::GUID => table.add_cell("GUID"),
            Column::Attributes => table.add_cell("Attributes"),
            Column::Name => table.add_cell("Name"),
        }
    }
    for (i, p) in gpt.iter().filter(|(_, x)| x.is_used()) {
        for column in opt.columns.iter() {
            match column {
                Column::Device => table.add_cell(&format!("{}{}", path.display(), i)),
                Column::Start => table.add_cell_rtl(&format!("{}", p.starting_lba)),
                Column::End => table.add_cell_rtl(&format!("{}", p.ending_lba)),
                Column::Sectors => table.add_cell_rtl(&format!("{}", p.size())),
                Column::Size => table.add_cell_rtl(&format_bytes(p.size() * gpt.sector_size)),
                Column::Type => table.add_cell(&format!(
                    "{}",
                    p.partition_type_guid.display_partition_type_guid()
                )),
                Column::GUID => {
                    table.add_cell(&format!("{}", p.unique_parition_guid.display_uuid()))
                }
                Column::Attributes => table.add_cell(&format!(
                    "{}",
                    p.attribute_bits
                        .display_attribute_bits(p.partition_type_guid)
                )),
                Column::Name => table.add_cell(&format!("{}", p.partition_name.as_str())),
            }
        }
    }
    print!("{}", table);

    Ok(())
}
