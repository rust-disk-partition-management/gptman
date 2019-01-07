use crate::cli::*;
use crate::types::TYPE_MAP;
use crate::uuid::convert_str_to_array;
use crate::uuid::UUID;

pub fn change_type<F>(gpt: &mut GPT, ask: &F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let i = ask_used_slot(gpt, ask)?;

    gpt[i].partition_type_guid = ask_partition_type_guid(ask)?;

    Ok(())
}

pub fn ask_partition_type_guid<F>(ask: &F) -> Result<[u8; 16]>
where
    F: Fn(&str) -> Result<String>,
{
    let mut categories: Vec<_> = TYPE_MAP.keys().collect();
    categories.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));

    loop {
        match ask("Partition type GUID (type L to list all types):")?.as_ref() {
            "" => {}
            "q" => break,
            "L" => loop {
                println!("Category:");
                for (i, cat) in categories.iter().enumerate() {
                    println!("{:2} => {}", i + 1, cat);
                }

                match ask("Choose category (q to go back):")?.as_ref() {
                    "" => {}
                    "q" => break,
                    i => loop {
                        if let Some(types_map) = usize::from_str_radix(i, 10)
                            .ok()
                            .and_then(|x| categories.get(x - 1))
                            .and_then(|x| TYPE_MAP.get(*x))
                        {
                            let mut types: Vec<_> = types_map.iter().collect();
                            types.sort_by(|a, b| a.1.cmp(b.1));
                            let types: Vec<(usize, &(&[u8; 16], &&str))> =
                                types.iter().enumerate().collect();

                            println!("Partition types:");
                            for (i, (guid, name)) in types.iter() {
                                println!("{:2} => {}: {}", i + 1, guid.display_uuid(), name);
                            }

                            match ask("Choose partition type (q to go back):")?.as_ref() {
                                "" => {}
                                "q" => break,
                                i => {
                                    if let Some(arr) = usize::from_str_radix(i, 10)
                                        .ok()
                                        .and_then(|x| types.get(x - 1).map(|(_, (arr, _))| **arr))
                                    {
                                        return Ok(arr);
                                    }
                                }
                            }
                        }
                    },
                }
            },
            x => match convert_str_to_array(x) {
                Ok(arr) => return Ok(arr),
                Err(err) => {
                    println!("{}", err);
                }
            },
        }
    }

    Err(Error::new("aborted."))
}
