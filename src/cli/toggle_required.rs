use crate::cli::*;
use crate::gpt::GPT;

pub fn toggle_required<F>(gpt: &mut GPT, ask: F) -> Result<()>
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

    gpt[i].attribute_bits ^= 0b1;

    Ok(())
}
