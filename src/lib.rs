//! Serde PHP serialization format support.
//!
//! See `README.md` for details.
//!
//! # Example
//!
//! ```rust
//! use serde::Deserialize;
//! use serde_php::from_bytes;
//!
//! #[derive(Debug, Deserialize, Eq, PartialEq)]
//! struct UserProfile {
//!     id: u32,
//!     name: String,
//!     tags: Vec<String>,
//! }
//!
//! let serialized = br#"a:3:{s:2:"id";i:42;s:4:"name";s:3:"Bob";s:4:"tags";a:2:{i:0;s:3:"foo";i:1;s:3:"bar";}}"#;
//!
//! let profile: UserProfile = from_bytes(serialized).expect("deserialization failed");
//! ```

pub mod de;
mod error;
pub mod ser;

pub use de::{from_bytes, PhpDeserializer};
pub use error::Error;
pub use ser::PhpSerializer;
