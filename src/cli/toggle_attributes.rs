use crate::cli::*;
use crate::gpt::GPT;

pub fn toggle_attributes<F>(gpt: &mut GPT, ask: F) -> Result<()>
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

    let attributes = loop {
        match ask("Enter GUID specific bits (48-63):")?.as_str() {
            "" => return Ok(()),
            s => {
                let attributes = s
                    .split(",")
                    .map(|x| u64::from_str_radix(x, 10))
                    .collect::<Vec<_>>();

                if let Some(attr) = attributes.iter().find(|x| x.is_err()) {
                    println!("{}", attr.as_ref().unwrap_err());
                } else if let Some(attr) = attributes
                    .iter()
                    .map(|x| x.as_ref().unwrap())
                    .find(|x| **x < 48 || **x > 63)
                {
                    println!("invalid attribute: {}", attr);
                } else {
                    break attributes.into_iter().map(|x| x.unwrap());
                }
            }
        }
    };

    for x in attributes {
        gpt[i].attribute_bits ^= 1 << x;
    }

    Ok(())
}
