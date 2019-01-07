use crate::cli::*;
use crate::gpt::GPT;
use std::path::PathBuf;

pub fn copy_partition<F>(dst_gpt: &mut GPT, ask: F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let src_path: PathBuf = ask("From disk:")?.as_str().into();
    let src_gpt = GPT::find_from(&mut fs::File::open(src_path)?)?;

    let default_src_i = src_gpt
        .iter()
        .filter(|(_, x)| x.is_used())
        .map(|(i, _)| i)
        .last()
        .ok_or(Error::new("no partition found"))?;
    let src_i = ask_with_default!(
        ask,
        |x| u32::from_str_radix(x, 10),
        "Source partition number",
        default_src_i
    )?;

    let default_dst_i = dst_gpt
        .iter()
        .filter(|(_, x)| x.is_unused())
        .map(|(i, _)| i)
        .next()
        .ok_or(Error::new("no available slot"))?;
    let dst_i = ask_with_default!(
        ask,
        |x| u32::from_str_radix(x, 10),
        "Target partition number",
        default_dst_i
    )?;

    if dst_gpt[dst_i].is_used() {
        println!("WARNING: partition {} is going to be overwritten", dst_i);
    }

    let size_in_bytes = src_gpt[src_i].size() * src_gpt.sector_size;
    if size_in_bytes % dst_gpt.sector_size != 0 {
        return Err(Error::new(&format!(
            "Partition size {} is not aligned to sector size {}",
            size_in_bytes, dst_gpt.sector_size
        )));
    }
    let size = size_in_bytes / dst_gpt.sector_size;

    let optimal_lba = dst_gpt
        .find_optimal_place(size)
        .ok_or(Error::new("not enough space on device"))?;
    let first_lba = dst_gpt.find_first_place(size).unwrap();
    let last_lba = dst_gpt.find_last_place(size).unwrap();
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

    dst_gpt[dst_i] = src_gpt[src_i].clone();
    dst_gpt[dst_i].starting_lba = starting_lba;
    dst_gpt[dst_i].ending_lba = starting_lba + size - 1;

    Ok(())
}
