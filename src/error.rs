//! Top-level error type for PHP serialization/deserialization.

use displaydoc::Display;
use std::{fmt, io};
use thiserror::Error;

/// Result type for PHP serialization/deserialization.
pub type Result<T> = ::core::result::Result<T, Error>;

/// PHP serialization/deserialization error.
#[derive(Error, Debug, Display)]
pub enum Error {
    /// Error serializing value: {0}
    WriteSerialized(io::Error),
    /// Feature not implemented by `serde_php`: {0}
    MissingFeature(&'static str),
    /// Attempted to serialize sequence of unknown length.
    ///
    /// PHP requires all collections to be length prefixed. Serializing
    /// sequences of unknown length requires writing these to a memory buffer
    /// with potentially unbounded space requirements and is thus disabled.
    LengthRequired,
    /// PHP Serialization error: {0}
    Custom(String),
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}
