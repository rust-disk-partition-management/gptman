use bincode::serialize_into;
use bincode::Result;
use std::io::{Seek, SeekFrom, Write};

pub fn write_protective_mbr_into<W: ?Sized>(mut writer: &mut W, sector_size: u64) -> Result<()>
where
    W: Write + Seek,
{
    let size = writer.seek(SeekFrom::End(0))? / sector_size - 1;
    writer.seek(SeekFrom::Start(446))?;
    // partition 1
    writer.write(&[
        0x00, // status
        0x00, 0x02, 0x00, // CHS address of first absolute sector
        0xee, // partition type
        0xff, 0xff, 0xff, // CHS address of last absolute sector
        0x01, 0x00, 0x00, 0x00, // LBA of first absolute sector
    ])?;
    // number of sectors in partition 1
    serialize_into(
        &mut writer,
        &(if size > u32::max_value() as u64 {
            u32::max_value()
        } else {
            size as u32
        }),
    )?;
    writer.write(&[0; 16])?; // partition 2
    writer.write(&[0; 16])?; // partition 3
    writer.write(&[0; 16])?; // partition 4
    writer.write(&[0x55, 0xaa])?; // signature
    println!("protective MBR has been written");

    Ok(())
}
