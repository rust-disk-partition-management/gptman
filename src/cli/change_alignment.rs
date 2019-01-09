use crate::cli::*;

pub fn change_alignment<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    gpt.align = ask_with_default!(
        ask,
        |x| u64::from_str_radix(x, 10),
        "Partition alignment",
        gpt.align
    )?;

    Ok(())
}
