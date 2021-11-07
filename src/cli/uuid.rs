use rand::Rng;
use std::fmt;
use std::num::ParseIntError;

#[derive(Debug)]
pub struct Error(String);

impl From<&ParseIntError> for Error {
    fn from(err: &ParseIntError) -> Error {
        Error(format!("{}", err))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)?;

        Ok(())
    }
}

pub trait Uuid {
    fn display_uuid(&self) -> String;
}

impl Uuid for [u8; 16] {
    fn display_uuid(&self) -> String {
        let mut digits: Vec<_> = self.iter().collect();
        let mut uuid: Vec<String> = Vec::new();
        uuid.extend(digits.drain(..4).rev().map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..2).rev().map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..2).rev().map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..2).map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..).map(|x| format!("{:02X}", x)));

        uuid.into_iter().collect()
    }
}

pub fn generate_random_uuid() -> [u8; 16] {
    rand::thread_rng().gen()
}

pub fn convert_str_to_array(uuid: &str) -> Result<[u8; 16], Error> {
    let mut arr = [0; 16];
    let mut digits: Vec<_> = uuid
        .chars()
        .filter(|&x| x != '-')
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|x| x.iter().collect::<String>())
        .map(|x| u8::from_str_radix(x.as_str(), 16))
        .collect();

    if digits.len() != 16 {
        return Err(Error(format!(
            "invalid number of digits ({} != 16)",
            digits.len()
        )));
    }

    let mut reordered = Vec::new();
    reordered.extend(digits.drain(..4).rev());
    reordered.extend(digits.drain(..2).rev());
    reordered.extend(digits.drain(..2).rev());
    reordered.extend(digits.drain(..2));
    #[allow(clippy::extend_with_drain)]
    reordered.extend(digits.drain(..));

    for (e, v) in arr.iter_mut().zip(reordered.iter()) {
        *e = *(v.as_ref()?);
    }

    Ok(arr)
}
