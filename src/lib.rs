//! Serde PHP serialization format support.
//!
//! See `README.md` for details.

pub mod de;

pub use de::{from_bytes, PhpDeserializer};
