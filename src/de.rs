//! PHP deserialization.

use serde::de::value::Error;
use serde::de::Error as ErrorTrait;
use serde::de::MapAccess;
use serde::de::{Deserialize, DeserializeSeed, IntoDeserializer, SeqAccess, Visitor};
use serde::{forward_to_deserialize_any, Deserializer};
use smallvec::SmallVec;
use std::io;
use std::io::{BufRead, Read};

/// Internal result type alias.
type Result<T> = core::result::Result<T, Error>;

/// Deserialize from byte slice.
pub fn from_bytes<'de, T>(s: &'de [u8]) -> Result<T>
where
    T: Deserialize<'de>,
{
    let buffered = io::BufReader::new(s);
    let mut des = PhpDeserializer::new(buffered);
    let value = T::deserialize(&mut des)?;
    Ok(value)
}

/// Lookahead buffer with integrated lexer.
///
/// Supports peeking ahead a single byte.
#[derive(Debug)]
struct Lookahead1<R> {
    reader: R,
    buffer: Option<u8>,
}

impl<R: Read> Lookahead1<R> {
    fn new(reader: R) -> Self {
        Lookahead1 {
            reader,
            buffer: None,
        }
    }

    /// Fill `buffer` with the next byte if there is one.
    ///
    /// Has no effect if `buffer` is already full.
    fn fill(&mut self) -> Result<()> {
        if self.buffer.is_none() {
            self.buffer = {
                let mut buf: [u8; 1] = [0];
                let length = self.reader.read(&mut buf).map_err(Error::custom)?;

                if length == 0 {
                    None
                } else {
                    Some(buf[0])
                }
            };
        }

        Ok(())
    }

    /// Peek at the next byte, without removing it. Returns `None` on EOF.
    fn peek(&mut self) -> Result<Option<u8>> {
        self.fill()?;
        Ok(self.buffer)
    }

    /// Reed a single byte, returning an error on EOF.
    fn read1(&mut self) -> Result<u8> {
        self.fill()?;

        self.buffer
            .take()
            .ok_or_else(|| Error::custom("Unexpected EOF"))
    }

    /// Expect a specific character.
    fn expect(&mut self, expected: u8) -> Result<()> {
        let actual = self.read1()?;
        if actual == expected {
            Ok(())
        } else {
            Err(Error::custom(format!(
                "Expected {:?}, got {:?} instead.",
                char::from(expected),
                char::from(actual)
            )))
        }
    }

    /// Reads an unsigned integer, fails on EOF and non-digit, but stops on
    /// the first invalid character after at least one digit has been read.
    fn collect_unsigned(&mut self, buf: &mut SmallVec<[u8; 32]>) -> Result<()> {
        // Read the first character and ensure it is a digit.
        let c = self.read1()?;
        if !c.is_ascii_digit() {
            return Err(Error::custom(format!(
                "Expected digit, got `{:?}`",
                char::from(c)
            )));
        }
        buf.push(c);

        // Keep reading digits until we hit EOF or a non-digit.
        while let Some(c) = self.peek()? {
            if !c.is_ascii_digit() {
                break;
            }
            self.expect(c)?;
            buf.push(c);
        }

        Ok(())
    }

    /// Read a `-` or `+` sign into a buffer, if present.
    fn collect_sign(&mut self, buf: &mut SmallVec<[u8; 32]>) -> Result<()> {
        match self.peek()? {
            Some(c @ b'+') | Some(c @ b'-') => {
                buf.push(c);
                self.expect(c)?;
            }
            _ => (),
        }

        Ok(())
    }

    /// Read raw PHP bytestring from input.
    fn read_raw_string(&mut self) -> Result<Vec<u8>> {
        // Thankfully, PHP strings are length-delimited, even though
        // they strangely enough include quotes as well.
        let mut buf = SmallVec::new();
        self.collect_unsigned(&mut buf)?;
        let length: usize = parse_bytes(buf)?;

        // Delim and opening quote:
        self.expect(b':')?;
        self.expect(b'"')?;

        // Inner string data. Note that this code will happily allocate
        // up to 4 GB of RAM on the heap.
        let mut data = vec![0; length];
        self.read_exact(&mut data)?;
        debug_assert!(data.len() == length);

        // Closing quote.
        self.expect(b'"')?;
        self.expect(b';')?;

        Ok(data)
    }

    /// Read an array header that follows after the `b"a:"` part.
    fn read_array_header(&mut self) -> Result<usize> {
        // Read number of elements.
        let mut buf = SmallVec::new();
        self.collect_unsigned(&mut buf)?;
        let num_elements = parse_bytes(buf)?;

        // Read opening part of array.
        self.expect(b':')?;
        self.expect(b'{')?;

        Ok(num_elements)
    }

    /// Read exactly defined number of bytes.
    fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<()> {
        // Bail early on zero-length strings.
        if buf.is_empty() {
            return Ok(());
        }

        // If we have buffered a character, move it to buf.
        if let Some(c) = self.buffer.take() {
            buf[0] = c;
            buf = &mut buf[1..];
        }

        // We can now read the remainder.
        self.reader.read_exact(buf).map_err(Error::custom)
    }
}

/// PHP deserializer.
///
/// Deserializes the format used by PHP's `serialize` function.
#[derive(Debug)]
pub struct PhpDeserializer<R> {
    input: Lookahead1<R>,
}

impl<R> PhpDeserializer<R>
where
    R: BufRead,
{
    fn new(input: R) -> PhpDeserializer<R> {
        PhpDeserializer {
            input: Lookahead1::new(input),
        }
    }
}

/// Parse a byte string using any `FromStr` function.
fn parse_bytes<E, T: std::str::FromStr<Err = E>, B: AsRef<[u8]>>(buf: B) -> Result<T>
where
    E: std::fmt::Display,
{
    let s = std::str::from_utf8(buf.as_ref()).map_err(Error::custom)?;
    s.parse().map_err(Error::custom)
}

impl<'a, 'de, R> Deserializer<'de> for &'a mut PhpDeserializer<R>
where
    R: BufRead,
{
    type Error = Error;

    fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // All fields start with a type, followed by a colon.
        let sym = self.input.read1()?;
        self.input.expect(b':')?;

        // See https://stackoverflow.com/questions/14297926/structure-of-a-serialized-php-string
        match sym {
            b'b' => {
                let val = self.input.read1()?;
                self.input.expect(b';')?;

                // Boolean.
                match val {
                    b'0' => visitor.visit_bool(false),
                    b'1' => visitor.visit_bool(true),
                    c => Err(Error::custom(format!("Invalid boolean symbol: {:?}", c))),
                }
            }
            b'i' => {
                // Integer.
                let mut buf = SmallVec::new();

                // Collect a potential sign, followed by the unsigned digits.
                self.input.collect_sign(&mut buf)?;
                self.input.collect_unsigned(&mut buf)?;

                // Terminating semicolon.
                self.input.expect(b';')?;

                // Finally, pass to visitor.
                visitor.visit_i64(parse_bytes(buf)?)
            }
            b'd' => {
                // Float.
                let mut buf = SmallVec::new();

                // Same as integer:
                self.input.collect_sign(&mut buf)?;
                self.input.collect_unsigned(&mut buf)?;

                // PHP omits decimal dots when serializing `.0` values.
                let dot = self.input.peek()?;

                if let Some(b'.') = dot {
                    buf.push(b'.');
                    self.input.expect(b'.')?;

                    // The remainder is another digit string without sign.
                    self.input.collect_unsigned(&mut buf)?;
                }

                self.input.expect(b';')?;

                visitor.visit_f64(parse_bytes(buf)?)
            }
            b's' => {
                // PHP String.

                let data = self.input.read_raw_string()?;

                // We now have the complete bytestring, no further parsing required.
                visitor.visit_seq(serde::de::value::SeqDeserializer::new(data.into_iter()))
            }
            b'N' => {
                // null.
                self.input.expect(b'N')?;
                self.input.expect(b';')?;
                visitor.visit_none()
            }
            b'a' => {
                // Array.
                let num_elements = self.input.read_array_header()?;

                // We support two ways of array deserialization: tuple and struct.
                //
                // Numeric arrays are deserialized as tuples and assumed to
                // contain no missing keys.
                //
                // Associative arrays must contain only string keys and are
                // serialized as mappings.
                //
                // Other variants are currently not supported and would require
                // hashmaps and variant types.

                let rval = match self.input.peek()? {
                    Some(b'i') | Some(b'}') => {
                        // Numeric or empty array.
                        visitor.visit_seq(ArraySequence::new(&mut self, num_elements))
                    }
                    Some(b's') => {
                        // Associative array.
                        visitor.visit_map(ArrayMapping::new(&mut self, num_elements))
                    }
                    _ => Err(Error::custom(
                        "Could not determine array type, should start with numeric or string key",
                    )),
                };
                self.input.expect(b'}')?;
                rval
            }
            b'O' => {
                // Object.
                Err(Error::custom(
                    "Object deserialization is not implemented, sorry.",
                ))
            }
            // Unknown character, not valid.
            c => Err(Error::custom(format!(
                "Unknown type indicator: {:?}",
                char::from(c)
            ))),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.input.expect(b's')?;
        self.input.expect(b':')?;
        // Actual UTF-8 strings are not a thing in PHP, but we offer this conversion
        // as a convenience.
        let raw = self.input.read_raw_string()?;
        visitor.visit_string(String::from_utf8(raw).map_err(Error::custom)?)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // A `null` value indicates our `None` here.
        if let Some(b'N') = self.input.peek()? {
            self.input.expect(b'N')?;
            self.input.expect(b';')?;
            visitor.visit_none()
        } else {
            // Otherwise, we can parse the actual value.
            visitor.visit_some(self)
        }
    }

    fn deserialize_struct<V>(
        mut self,
        _name: &str,
        _fields: &[&str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // We need to explicitly implement struct deserialization to be able
        // to distinguish between empty numeric arrays and empty associative
        // arrays.
        self.input.expect(b'a')?;
        self.input.expect(b':')?;
        let num_elements = self.input.read_array_header()?;
        let rval = visitor.visit_map(ArrayMapping::new(&mut self, num_elements));
        self.input.expect(b'}')?;

        rval
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
        bytes byte_buf unit unit_struct newtype_struct seq tuple
         map enum identifier ignored_any tuple_struct
    }
}

/// Numeric array sequence helper.
#[derive(Debug)]
struct ArraySequence<'a, R> {
    de: &'a mut PhpDeserializer<R>,
    num_elements: usize,
    index: usize,
}

impl<'a, R> ArraySequence<'a, R> {
    fn new(de: &'a mut PhpDeserializer<R>, num_elements: usize) -> Self {
        ArraySequence {
            de,
            num_elements,
            index: 0,
        }
    }
}

impl<'a, 'de, R> SeqAccess<'de> for ArraySequence<'a, R>
where
    R: BufRead,
{
    type Error = Error;

    fn size_hint(&self) -> Option<usize> {
        Some(self.num_elements - self.index)
    }

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if self.num_elements == self.index {
            return Ok(None);
        }

        // Get the index; we are assuming to have a PHP array in regular
        // "array style", that is with only numerical keys stored in order.
        //
        // TODO: Possibly change this behavior to handle arrays with out-of-order keys.
        let idx = usize::deserialize(&mut *self.de)?;
        if idx != self.index {
            return Err(Error::custom(format!(
                "PHP array index mismatched. Expected index {}, but found {}",
                self.index, idx
            )));
        }
        debug_assert_eq!(idx, self.index);
        self.index += 1;

        // We can now deserialize the actual value.
        seed.deserialize(&mut *self.de).map(Some)
    }
}

/// Associative array helper.
#[derive(Debug)]
struct ArrayMapping<'a, R> {
    de: &'a mut PhpDeserializer<R>,
    num_elements: usize,
    index: usize,
}

impl<'a, R> ArrayMapping<'a, R> {
    fn new(de: &'a mut PhpDeserializer<R>, num_elements: usize) -> Self {
        ArrayMapping {
            de,
            num_elements,
            index: 0,
        }
    }
}

impl<'a, 'de, R> MapAccess<'de> for ArrayMapping<'a, R>
where
    R: BufRead,
{
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        // We are keeping count, so no need to check for end delimiting symbols.
        if self.index == self.num_elements {
            return Ok(None);
        }

        // We need to hint that we are deserializing a string, since PHP
        // strings are not fit to be keys. For this reason, we perform the
        // deserialization here:
        let key = String::deserialize(&mut *self.de)?;

        // Pass the already deserialized string on.
        seed.deserialize(key.into_deserializer()).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        self.index += 1;
        seed.deserialize(&mut *self.de)
    }
}

#[cfg(test)]
mod tests {
    use super::from_bytes;
    use serde::Deserialize;
    use std::collections::HashMap;

    #[test]
    fn deserialize_bool() {
        assert_eq!(from_bytes(b"b:0;"), Ok(false));
        assert_eq!(from_bytes(b"b:1;"), Ok(true));
    }

    #[test]
    fn deserialize_integer() {
        assert_eq!(from_bytes(b"i:-1;"), Ok(-1i64));
        assert_eq!(from_bytes(b"i:0;"), Ok(0i64));
        assert_eq!(from_bytes(b"i:1;"), Ok(1i64));
        assert_eq!(from_bytes(b"i:123;"), Ok(123i64));
    }

    #[test]
    fn deserialize_float() {
        assert_eq!(from_bytes(b"d:-1;"), Ok(-1f64));
        assert_eq!(from_bytes(b"d:0;"), Ok(0f64));
        assert_eq!(from_bytes(b"d:1;"), Ok(1f64));
        assert_eq!(from_bytes(b"d:-1.9;"), Ok(-1.9f64));
        assert_eq!(from_bytes(b"d:0.9;"), Ok(0.9f64));
        assert_eq!(from_bytes(b"d:1.9;"), Ok(1.9f64));
    }

    #[test]
    fn deserialize_php_string() {
        let input = br#"s:14:"single quote '";"#;
        assert_eq!(
            from_bytes::<Vec<u8>>(input).unwrap(),
            b"single quote '".to_owned()
        );
    }

    #[test]
    fn deserialize_string() {
        let input = br#"s:14:"single quote '";"#;
        assert_eq!(
            from_bytes::<String>(input).unwrap(),
            "single quote '".to_owned()
        );
    }

    #[test]
    fn deserialize_array() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct SubData();

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Data(Vec<u8>, Vec<u8>, SubData);

        let input = br#"a:3:{i:0;s:4:"user";i:1;s:0:"";i:2;a:0:{}}"#;
        assert_eq!(
            from_bytes::<Data>(input).unwrap(),
            Data(b"user".to_vec(), b"".to_vec(), SubData())
        );
    }

    #[test]
    fn deserialize_struct() {
        // PHP equiv:
        //
        // array("foo" => true,
        //       "bar" => "xyz",
        //       "sub" => array("x" => 42))

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Outer {
            foo: bool,
            bar: String,
            sub: Inner,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Inner {
            x: i64,
        }

        let input = br#"a:3:{s:3:"foo";b:1;s:3:"bar";s:3:"xyz";s:3:"sub";a:1:{s:1:"x";i:42;}}"#;
        let expected = Outer {
            foo: true,
            bar: "xyz".to_owned(),
            sub: Inner { x: 42 },
        };

        assert_eq!(from_bytes(input), Ok(expected));
    }

    #[test]
    fn deserialize_struct_with_optional() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Location {
            province: Option<String>,
            postalcode: Option<String>,
            country: Option<String>,
        }

        let input_a = br#"a:0:{}"#;
        let input_b = br#"a:1:{s:8:"province";s:29:"Newfoundland and Labrador, CA";}"#;
        let input_c =
            br#"a:2:{s:10:"postalcode";s:5:"90002";s:7:"country";s:24:"United States of America";}
"#;

        let expected_a = Location {
            province: None,
            postalcode: None,
            country: None,
        };
        let expected_b = Location {
            province: Some("Newfoundland and Labrador, CA".to_owned()),
            postalcode: None,
            country: None,
        };
        let expected_c = Location {
            province: None,
            postalcode: Some("90002".to_owned()),
            country: Some("United States of America".to_owned()),
        };
        assert_eq!(from_bytes(input_a), Ok(expected_a));
        assert_eq!(from_bytes(input_b), Ok(expected_b));
        assert_eq!(from_bytes(input_c), Ok(expected_c));
    }

    #[test]
    fn deserialize_nested() {
        // PHP: array("x" => array("inner" => 1), "y" => array("inner" => 2))
        let input = br#"a:2:{s:1:"x";a:1:{s:5:"inner";i:1;}s:1:"y";a:1:{s:5:"inner";i:2;}}"#;

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Outer {
            x: Inner,
            y: Inner,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Inner {
            inner: u8,
        }

        let expected = Outer {
            x: Inner { inner: 1 },
            y: Inner { inner: 2 },
        };

        assert_eq!(from_bytes(input), Ok(expected));
    }

    #[test]
    fn deserialize_variable_length() {
        // PHP: array(1.1, 2.2, 3.3, 4.4)
        let input = br#"a:4:{i:0;d:1.1;i:1;d:2.2;i:2;d:3.3;i:3;d:4.4;}"#;

        let expected = vec![1.1, 2.2, 3.3, 4.4];
        assert_eq!(from_bytes(input), Ok(expected));
    }

    #[test]
    fn deserialize_hashmap() {
        // PHP: array("foo" => 1, "bar" => 2)
        let input = br#"a:2:{s:3:"foo";i:1;s:3:"bar";i:2;}"#;
        let mut expected: HashMap<String, u16> = HashMap::new();
        expected.insert("foo".to_owned(), 1);
        expected.insert("bar".to_owned(), 2);

        assert_eq!(from_bytes(input), Ok(expected));
    }
}