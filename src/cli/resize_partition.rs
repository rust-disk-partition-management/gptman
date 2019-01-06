use crate::cli::*;
use crate::gpt::GPT;

pub fn resize_partition<F>(gpt: &mut GPT, ask: F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let default_i = gpt
        .iter()
        .filter(|(_, x)| x.is_used())
        .map(|(i, _)| i)
        .last()
        .ok_or(Error::new("no partition found"))?;

    let i = ask_with_default!(
        ask,
        |x| u32::from_str_radix(x, 10),
        "Partition number",
        default_i
    )?;

    let free_sectors = gpt.find_free_sectors();
    let mut p = &mut gpt[i];

    let max_size: u64 = p.ending_lba - p.starting_lba
        + 1
        + free_sectors
            .iter()
            .skip_while(|(i, _)| *i < p.starting_lba)
            .take(1)
            .filter(|(i, _)| *i == p.ending_lba + 1)
            .next()
            .map(|(_, l)| l)
            .unwrap_or(&0);

    let size = loop {
        match ask_with_default!(
            ask,
            |x| u64::from_str_radix(x, 10),
            "Partition size",
            max_size
        )? {
            0 => println!("The size must be at least 1 sector"),
            x if x > max_size => println!("The maximum size is {}", max_size),
            x => break x,
        }
    };

    p.ending_lba = p.starting_lba + size - 1;

    Ok(())
}
