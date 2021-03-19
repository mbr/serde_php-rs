//! Top-level error type for PHP serialization/deserialization.

use std::{fmt, io};

/// Result type for PHP serialization/deserialization.
pub type Result<T> = ::core::result::Result<T, Error>;

/// PHP serialization/deserialization error.
#[derive(Debug)]
pub enum Error {
    /// Error writing serialized value.
    WriteSerialized(io::Error),
    /// Error reading serializing value.
    ReadSerialized(io::Error),
    /// Unexpected end of file while reading.
    UnexpectedEof,
    /// Unexpected input.
    Unexpected {
        /// Byte expected.
        expected: char,
        /// Actual byte found.
        actual: char,
    },
    /// Expected a digit, but got non-digit value instead.
    ExpectedDigit {
        /// Non-digit found.
        actual: char,
    },
    /// Deserialized bytestring is not valid UTF.
    Utf8Error(std::str::Utf8Error),
    /// Could not convert into char from decimal value.
    CharConversionFailed(std::char::CharTryFromError),
    /// Not a valid number or incorrect number type.
    NotAValidNumber(Box<dyn std::error::Error + Send + Sync>),
    /// Not a valid value for boolean.
    InvalidBooleanValue(char),
    /// Unsupported array key type: must be all strings or all numeric.
    UnsupportedArrayKeyType(char),
    /// Invalid type indicator on value.
    InvalidTypeIndicator(char),
    /// Feature not implemented by `serde_php`.
    MissingFeature(&'static str),
    /// Array-index mismatch: must be in-order and numeric.
    IndexMismatch {
        /// Expected index.
        expected: usize,
        /// Actual index found.
        actual: usize,
    },
    /// Attempted to serialize sequence of unknown length.
    ///
    /// PHP requires all collections to be length prefixed. Serializing
    /// sequences of unknown length requires writing these to a memory buffer
    /// with potentially unbounded space requirements and is thus disabled.
    LengthRequired,
    /// PHP Deserialization failed.
    SerializationFailed(String),
    /// PHP Serialization failed.
    DeserializationFailed(String),
}

// Note: Manual error implementation as opposed to `thiserror`, otherwise
//       `NotAValidNumber` errors cannot be constructed `Send`.
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::WriteSerialized(ref err) => Some(err),
            Error::ReadSerialized(ref err) => Some(err),
            Error::Utf8Error(ref err) => Some(err),
            Error::CharConversionFailed(ref err) => Some(err),
            Error::NotAValidNumber(ref err) => Some(err.as_ref()),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[allow(clippy::enum_glob_use)]
        use Error::*;

        match self {
            WriteSerialized(err) => write!(f, "Error writing serialized value: {}", err),
            ReadSerialized(err) => write!(f, "Error reading serializing value: {}", err),
            UnexpectedEof => write!(f, "Unexpected end of file while reading"),
            Unexpected { expected, actual } => {
                write!(f, "Expected `{}` but got `{}` instead", expected, actual)
            }
            ExpectedDigit { actual } => write!(f, "Expected a digit, but got `{}` instead", actual),
            Utf8Error(err) => write!(f, "Deserialized bytestring is not valid UTF: {}", err),
            CharConversionFailed(err) => {
                write!(f, "Could not convert into char from decimal value: {}", err)
            }
            NotAValidNumber(err) => {
                write!(f, "Not a valid number or incorrect number type: {}", err)
            }
            InvalidBooleanValue(ch) => write!(f, "Not a valid value for boolean: {}", ch),
            UnsupportedArrayKeyType(ch) => write!(f, "Unsupported array key type: {}", ch),
            InvalidTypeIndicator(ch) => write!(f, "Invalid type indicator on value: {}", ch),
            MissingFeature(feat) => write!(f, "Feature not implemented by `serde_php`: {}", feat),
            IndexMismatch { expected, actual } => write!(
                f,
                "Array-index mismatch, expected {} but got {}",
                expected, actual
            ),
            LengthRequired => write!(f, "Attempted to serialize sequence of unknown length"),
            SerializationFailed(err) => write!(f, "PHP Deserialization failed: {}", err),
            DeserializationFailed(err) => write!(f, "PHP Serialization failed: {}", err),
        }
    }
}

impl serde::ser::Error for Error {
    #[inline]
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::SerializationFailed(msg.to_string())
    }
}

impl serde::de::Error for Error {
    #[inline]
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::DeserializationFailed(msg.to_string())
    }
}
