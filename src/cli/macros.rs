macro_rules! main_unwrap {
    ($e:expr) => {{
        match $e {
            Ok(x) => x,
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        }
    }};
}

macro_rules! ask_with_default {
    ($ask:expr, $parser:expr, $prompt:expr, $default:expr) => {
        loop {
            let input = $ask(&format!("{} (default {}):", $prompt, $default))?;

            if input == "" {
                break Ok($default);
            } else {
                match $parser(&input) {
                    Err(err) => {
                        println!("{}", err);
                    }
                    x => break x,
                }
            }
        }
    };
}
