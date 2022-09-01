use crate::uuid;
use std::error::Error as ErrorTrait;
use std::fmt;
use std::io;

#[derive(Debug, PartialEq, Eq)]
pub struct Error(String);

impl Error {
    pub fn new(s: &str) -> Error {
        Error(s.to_string())
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Error {
        Error(s.to_string())
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(err: std::num::ParseIntError) -> Error {
        Error(format!("{}", err))
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error(format!("{}", err))
    }
}

impl From<gptman::Error> for Error {
    fn from(err: gptman::Error) -> Error {
        let mut text = format!("{}", err);
        let mut cause = err.source();
        while let Some(error) = cause {
            text.push_str(format!(": {}", error).as_str());
            cause = error.source();
        }

        Error(text)
    }
}

impl From<uuid::Error> for Error {
    fn from(err: uuid::Error) -> Error {
        Error(format!("{}", err))
    }
}

impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Error {
        Error(format!("{}", err))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)?;

        Ok(())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
