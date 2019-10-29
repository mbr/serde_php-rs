use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    Custom(String),
    WriteSerialized(io::Error),
    MissingFeature(&'static str),
    LengthRequired,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TODO")
    }
}

impl std::error::Error for Error {}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}
