use crate::cli::*;
use crate::gpt::GPT;
use std::path::PathBuf;

pub fn copy_all_partitions<F>(dst_gpt: &mut GPT, dst_path: &PathBuf, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let src_path: PathBuf =
        match ask(&format!("From disk (default {}):", dst_path.display()))?.as_str() {
            "" => dst_path.clone(),
            x => x.into(),
        };
    let src_gpt = GPT::find_from(&mut fs::File::open(src_path)?)?;

    for (src_i, p) in src_gpt.iter().filter(|(_, x)| x.is_used()) {
        let size_in_bytes = p.size() * src_gpt.sector_size;
        if size_in_bytes % dst_gpt.sector_size != 0 {
            return Err(Error::new(&format!(
                "Partition size {} is not aligned to sector size {}",
                size_in_bytes, dst_gpt.sector_size
            )));
        }
        let size = size_in_bytes / dst_gpt.sector_size;

        println!(
            "Copy partition {} of {} sectors ({}):",
            src_i,
            size,
            format_bytes(size_in_bytes)
        );
        let dst_i = ask_free_slot(dst_gpt, ask)?;
        let starting_lba = ask_starting_lba(dst_gpt, ask, size)?;

        dst_gpt[dst_i] = p.clone();
        dst_gpt[dst_i].starting_lba = starting_lba;
        dst_gpt[dst_i].ending_lba = starting_lba + size - 1;
    }

    Ok(())
}
