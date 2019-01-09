use crate::cli::*;
use crate::gpt::GPT;
use std::path::PathBuf;

pub fn copy_partition<F>(dst_gpt: &mut GPT, dst_path: &PathBuf, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let src_path: PathBuf =
        match ask(&format!("From disk (default {}):", dst_path.display()))?.as_str() {
            "" => dst_path.clone(),
            x => x.into(),
        };
    let src_gpt = GPT::find_from(&mut fs::File::open(src_path)?)?;

    let src_i = ask_used_slot(&src_gpt, ask)?;
    let dst_i = ask_free_slot(dst_gpt, ask)?;

    let size_in_bytes = src_gpt[src_i].size() * src_gpt.sector_size;
    if size_in_bytes % dst_gpt.sector_size != 0 {
        return Err(Error::new(&format!(
            "Partition size {} is not aligned to sector size {}",
            size_in_bytes, dst_gpt.sector_size
        )));
    }
    let size = size_in_bytes / dst_gpt.sector_size;

    let starting_lba = ask_starting_lba(dst_gpt, ask, size)?;

    dst_gpt[dst_i] = src_gpt[src_i].clone();
    dst_gpt[dst_i].starting_lba = starting_lba;
    dst_gpt[dst_i].ending_lba = starting_lba + size - 1;

    Ok(())
}
