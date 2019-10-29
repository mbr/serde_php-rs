#![forbid(missing_docs)]
//! # PHP serialization format support for serde
//!
//! PHP uses a custom serialization format through its `serialize` and `unserialize` methods. This crate adds partial support for this format using serde.
//!
//! An overview of the format can be seen at
//! <https://stackoverflow.com/questions/14297926/structure-of-a-serialized-php-string>,
//! details are available at
//! <http://www.phpinternalsbook.com/php5/classes_objects/serialization.html>.
//!
//! ## What is supported?
//!
//! * Basic and compound types:
//!
//!   | PHP type                | Rust type                                             |
//!   | ---                     | ---                                                   |
//!   | boolean                 | `bool`                                                |
//!   | integer                 | `i64` (automatic conversion to other types supported) |
//!   | float                   | `f64` (automatic conversion to `f32` supported)       |
//!   | strings                 | `Vec<u8>` (PHP strings are not UTF8)                  |
//!   | null                    | decoded as `None`                                     |
//!   | array (non-associative) | tuple `struct`s or `Vec<_>`                           |
//!   | array (associative)     | regular `struct`s or `HashMap<_, _>`                  |
//!
//! * Rust `String`s are transparently UTF8-converted to PHP bytestrings.
//!
//! ## What is missing?
//!
//! * PHP objects
//! * Out-of-order numeric arrays
//! * Non-string/numeric array keys, except when deserializing into a `HashMap`
//! * Mixed arrays. Array keys are assumed to always have the same key type
//!   (Note: If this is required, consider extending this library with a variant
//!    type).
//!
//! ## Example use
//!
//! Given an example data structure storing a session token using the following
//! PHP code
//!
//! ```php
//! <?php
//! $serialized = serialize(array("user", "", array()));
//! echo($serialized);
//! ```
//!
//! and thus the following output
//!
//! ```text
//! a:3:{i:0;s:4:"user";i:1;s:0:"";i:2;a:0:{}}
//! ```
//!
//! , the data can be reconstructed using the following rust code:
//!
//! ```rust
//! use serde::Deserialize;
//! use serde_php::from_bytes;
//!
//! #[derive(Debug, Deserialize, Eq, PartialEq)]
//! struct Data(Vec<u8>, Vec<u8>, SubData);
//!
//! #[derive(Debug, Deserialize, Eq, PartialEq)]
//! struct SubData();
//!
//! let input = br#"a:3:{i:0;s:4:"user";i:1;s:0:"";i:2;a:0:{}}"#;
//! assert_eq!(
//!     from_bytes::<Data>(input).unwrap(),
//!     Data(b"user".to_vec(), b"".to_vec(), SubData())
//! );
//! ```
//!
//! Likewise, structs are supported as well, if the PHP arrays use keys:
//!
//! ```php
//! <?php
//! $serialized = serialize(
//!     array("foo" => true,
//!           "bar" => "xyz",
//!           "sub" => array("x" => 42))
//! );
//! echo($serialized);
//! ```
//!
//! In Rust:
//!
//! ```rust
//!# use serde::Deserialize;
//!# use serde_php::from_bytes;
//! #[derive(Debug, Deserialize, Eq, PartialEq)]
//! struct Outer {
//!     foo: bool,
//!     bar: String,
//!     sub: Inner,
//! }
//!
//! #[derive(Debug, Deserialize, Eq, PartialEq)]
//! struct Inner {
//!     x: i64,
//! }
//!
//! let input = br#"a:3:{s:3:"foo";b:1;s:3:"bar";s:3:"xyz";s:3:"sub";a:1:{s:1:"x";i:42;}}"#;
//! let expected = Outer {
//!     foo: true,
//!     bar: "xyz".to_owned(),
//!     sub: Inner { x: 42 },
//! };
//!
//! let deserialized: Outer = from_bytes(input).expect("deserialization failed");
//!
//! assert_eq!(deserialized, expected);
//! ```
//!
//! ### Optional values
//!
//! Missing values can be left optional, as in this example:
//!
//! ```php
//! <?php
//! $location_a = array();
//! $location_b = array("province" => "Newfoundland and Labrador, CA");
//! $location_c = array("postalcode" => "90002",
//!                     "country" => "United States of America");
//! echo(serialize($location_a) . "\n");
//! echo(serialize($location_b) . "\n");
//! # -> a:1:{s:8:"province";s:29:"Newfoundland and Labrador, CA";}
//! echo(serialize($location_c) . "\n");
//! # -> a:2:{s:10:"postalcode";s:5:"90002";s:7:"country";
//! #         s:24:"United States of America";}
//! ```
//!
//! The following declaration of `Location` will be able to parse all three
//! example inputs.
//!
//! ```rust
//!# use serde::Deserialize;
//! #[derive(Debug, Deserialize, Eq, PartialEq)]
//! struct Location {
//!     province: Option<String>,
//!     postalcode: Option<String>,
//!     country: Option<String>,
//! }
//! ```
//!
//! # Full roundtrip example
//!
//! ```rust
//! use serde::{Deserialize, Serialize};
//! use serde_php::{to_vec, from_bytes};
//!
//! #[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
//! struct UserProfile {
//!     id: u32,
//!     name: String,
//!     tags: Vec<String>,
//! }
//!
//! let orig = UserProfile {
//!     id: 42,
//!     name: "Bob".to_owned(),
//!     tags: vec!["foo".to_owned(), "bar".to_owned()],
//! };
//!
//! let serialized = to_vec(&orig).expect("serialization failed");
//! let expected = br#"a:3:{s:2:"id";i:42;s:4:"name";s:3:"Bob";s:4:"tags";a:2:{i:0;s:3:"foo";i:1;s:3:"bar";}}"#;
//! assert_eq!(serialized, &expected[..]);
//!
//! let profile: UserProfile = from_bytes(&serialized).expect("deserialization failed");
//! assert_eq!(profile, orig);
//! ```

mod de;
mod error;
mod ser;

pub use de::from_bytes;
pub use error::{Error, Result};
pub use ser::{to_vec, to_writer};

#[cfg(test)]
mod tests {
    use super::{from_bytes, to_vec};
    use proptest::prelude::any;
    use proptest::proptest;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    macro_rules! roundtrip {
        ($ty:ty, $value:expr) => {
            let val = $value;

            let serialized = to_vec(&val).expect("Serialization failed");
            eprintln!("{}", String::from_utf8_lossy(serialized.as_slice()));

            let deserialized: $ty =
                from_bytes(serialized.as_slice()).expect("Deserialization failed");

            assert_eq!(deserialized, val);
        };
    }

    #[test]
    fn roundtrip_newtype() {
        #[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
        struct MyNewtype(i32);

        roundtrip!(MyNewtype, MyNewtype(0));
        roundtrip!(MyNewtype, MyNewtype(1));
        roundtrip!(MyNewtype, MyNewtype(-1));
    }

    proptest! {
        #[test]
        fn roundtrip_unit(v in any::<()>()) {
            roundtrip!((), v);
        }

        #[test]
        fn roundtrip_bool(v in any::<bool>()) {
            roundtrip!(bool, v);
        }

        #[test]
        fn roundtrip_u8(v in any::<u8>()) {
            roundtrip!(u8, v);
        }

        #[test]
        fn roundtrip_u16(v in any::<u16>()) {
            roundtrip!(u16, v);
        }

        #[test]
        fn roundtrip_u32(v in any::<u32>()) {
            roundtrip!(u32, v);
        }

        #[test]
        fn roundtrip_u64(v in 0..(std::i64::MAX as u64)) {
            roundtrip!(u64, v);
        }

        #[test]
        fn roundtrip_i8(v in any::<i8>()) {
            roundtrip!(i8, v);
        }

        #[test]
        fn roundtrip_i16(v in any::<i16>()) {
            roundtrip!(i16, v);
        }

        #[test]
        fn roundtrip_i32(v in any::<i32>()) {
            roundtrip!(i32, v);
        }

        #[test]
        fn roundtrip_i64(v in any::<i64>()) {
            roundtrip!(i64, v);
        }

        #[test]
        fn roundtrip_f32(v in any::<f32>()) {
            roundtrip!(f32, v);
        }

        #[test]
        fn roundtrip_f64(v in any::<f64>()) {
            roundtrip!(f64, v);
        }

        #[test]
        fn roundtrip_bytes(v in any::<Vec<u8>>()) {
            roundtrip!(Vec<u8>, v);
        }

        #[test]
        fn roundtrip_char(v in any::<char>()) {
            roundtrip!(char, v);
        }

        #[test]
        fn roundtrip_string(v in any::<String>()) {
            roundtrip!(String, v);
        }

        #[test]
        fn roundtrip_option(v in any::<Option<i32>>()) {
            roundtrip!(Option<i32>, v);
        }

        #[test]
        fn roundtrip_same_type_tuple(v in any::<(u32, u32)>()) {
            roundtrip!((u32, u32), v);
        }

        #[test]
        fn roundtrip_mixed_type_tuple(v in any::<(String, i32)>()) {
            roundtrip!((String, i32), v);
        }

        #[test]
        fn roundtrip_string_string_hashmap(v in proptest::collection::hash_map(any::<String>(), any::<String>(), 0..100)) {
            roundtrip!(HashMap<String, String>, v);
        }
    }
}
