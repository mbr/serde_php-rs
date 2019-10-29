//! Top-level error type for PHP serialization/deserialization.

use displaydoc::Display;
use std::{fmt, io};
use thiserror::Error;

/// Result type for PHP serialization/deserialization.
pub type Result<T> = ::core::result::Result<T, Error>;

/// PHP serialization/deserialization error.
#[derive(Error, Debug, Display)]
pub enum Error {
    /// Error writing serializated value: {0}
    WriteSerialized(#[source] io::Error),
    /// Error reading serializing value: {0}
    ReadSerialized(#[source] io::Error),
    /// Unexpected end of file while reading,
    UnexpectedEof,
    /// Expected `{expected}` but got `{actual}` instead.
    Unexpected {
        /// Byte expected.
        expected: char,
        /// Actual byte found.
        actual: char,
    },
    /// Expected a digit, but got `{actual}` instead.
    ExpectedDigit {
        /// Non-digit found.
        actual: char,
    },
    /// Deserialized bytestring is not valid UTF: {0}
    Utf8Error(#[source] std::str::Utf8Error),
    /// Not a valid number or incorrect number type: {0}
    NotAValidNumber(#[source] Box<std::error::Error>),
    /// Not a valid value for boolean: {0}
    InvalidBooleanValue(char),
    /// Unsupported array key type (must be all strings or all numeric): {0}
    UnsupportedArrayKeyType(char),
    /// Invalid type indicator on value: {0}
    InvalidTypeIndicator(char),
    /// Feature not implemented by `serde_php`: {0}
    MissingFeature(&'static str),
    /// Array-index mismatch (must be in-order and numeric), expected {expected}
    /// but got {actual}
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
    /// PHP Deserialization failed: {0}
    SerializationFailed(String),
    /// PHP Serialization failed: {0}
    DeserializationFailed(String),
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
