use crate::error::{Error, Result};
use serde::{ser, Serialize};
use std::io::Write;

/// Write out serialization of value.
#[inline]
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
where
    W: Write,
    T: Serialize + ?Sized,
{
    let mut ser = PhpSerializer::new(writer);
    value.serialize(&mut ser)
}

/// Write serialization of value into byte vector.
#[inline]
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize + ?Sized,
{
    let mut buf = Vec::new();
    to_writer(&mut buf, value)?;
    Ok(buf)
}

/// Central serializer structure.
#[derive(Debug)]
struct PhpSerializer<W> {
    output: W,
}

impl<W> PhpSerializer<W> {
    /// Create new serializer on writer.
    #[inline]
    fn new(output: W) -> Self {
        PhpSerializer { output }
    }
}

impl<'a, W> ser::Serializer for &'a mut PhpSerializer<W>
where
    W: Write,
{
    type Ok = ();

    type Error = Error;

    type SerializeSeq = NumericArraySerializer<'a, W>;
    type SerializeTuple = NumericArraySerializer<'a, W>;
    type SerializeTupleStruct = NumericArraySerializer<'a, W>;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<()> {
        if v {
            self.output.write_all(b"b:1;")
        } else {
            self.output.write_all(b"b:0;")
        }
        .map_err(Error::WriteSerialized)
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<()> {
        // We rely on Rust having a "standard" display implementation for
        // `i64` types, which is a reasonable assumption.
        write!(self.output, "i:{};", v).map_err(Error::WriteSerialized)
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<()> {
        write!(self.output, "i:{};", v).map_err(Error::WriteSerialized)
    }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<()> {
        self.serialize_f64(f64::from(v))
    }

    #[inline]
    fn serialize_f64(self, v: f64) -> Result<()> {
        // Float representations _should_ match up.
        // TODO: Verify this prints edges correctly.
        write!(self.output, "d:{};", v).map_err(Error::WriteSerialized)
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_u32(u32::from(v))
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<()> {
        self.serialize_bytes(v.as_bytes())
    }

    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        write!(self.output, "s:{}:\"", v.len()).map_err(Error::WriteSerialized)?;
        self.output.write_all(v).map_err(Error::WriteSerialized)?;
        write!(self.output, "\";").map_err(Error::WriteSerialized)
    }

    #[inline]
    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<()> {
        self.output.write_all(b"N;").map_err(Error::WriteSerialized)
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(Error::MissingFeature(
            "Serialization of unit structures is not supported.",
        ))
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Err(Error::MissingFeature(
            "Serialization of enums is not supported. If you need C-style enums serialized, look at `serde_repr`.",
        ))
    }

    #[inline]
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // We just "unpack" newtypes when deserializing.
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::MissingFeature(
            "Serialization of enums is not supported. If you need C-style enums serialized, look at `serde_repr`.",
        ))
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        // Sequence serialization is iffy because we would need to buffer
        // the whole serialized string in memory if we do not know the number
        // of elements in the sequence.
        //
        // We return an error instead if the length is not known, as this is
        // preferrable to writing multi-megabyte strings into memory by
        // accident.
        if let Some(n) = len {
            // We can assume sequences are all of the same type.
            write!(self.output, "a:{}:{{", n).map_err(Error::WriteSerialized)?;
            Ok(NumericArraySerializer::new(self))
        } else {
            return Err(Error::LengthRequired);
        }
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_tuple(len)
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::MissingFeature(
            "Serialization of enums is not supported. If you need C-style enums serialized, look at `serde_repr`.",
        ))
    }

    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        if let Some(n) = len {
            write!(self.output, "a:{}:{{", n).map_err(Error::WriteSerialized)?;
            // No need to count elements, thus no added state.
            Ok(self)
        } else {
            return Err(Error::LengthRequired);
        }
    }

    #[inline]
    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::MissingFeature(
            "Serialization of enums is not supported. If you need C-style enums serialized, look at `serde_repr`.",
        ))
    }
}

/// Helper structure for numeric arrays.
#[derive(Debug)]
pub struct NumericArraySerializer<'a, W> {
    // There is no delimiter for elements (arrays are length-prefixed and
    // and carry their own terminator. However, we still need to count
    // the elements.
    index: usize,
    serializer: &'a mut PhpSerializer<W>,
}

impl<'a, W> NumericArraySerializer<'a, W> {
    /// Create new numeric array helper.
    fn new(serializer: &'a mut PhpSerializer<W>) -> Self {
        NumericArraySerializer {
            index: 0,
            serializer,
        }
    }
}

impl<'a, W> ser::SerializeSeq for NumericArraySerializer<'a, W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let mut ser = PhpSerializer::new(&mut self.serializer.output);

        // Output-format is just index directly followed by value.
        self.index.serialize(&mut ser)?;
        value.serialize(&mut ser)?;
        self.index += 1;
        Ok(())
    }

    fn end(self) -> Result<()> {
        self.serializer
            .output
            .write_all(b"}")
            .map_err(Error::WriteSerialized)
    }
}

impl<'a, W> ser::SerializeTuple for NumericArraySerializer<'a, W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W> ser::SerializeTupleStruct for NumericArraySerializer<'a, W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W> ser::SerializeTupleVariant for &'a mut PhpSerializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::MissingFeature(
            "Serialization of enums is not supported. If you need C-style enums serialized, look at `serde_repr`.",
        ))
    }

    fn end(self) -> Result<()> {
        Err(Error::MissingFeature(
            "Serialization of enums is not supported. If you need C-style enums serialized, look at `serde_repr`.",
        ))
    }
}

impl<'a, W> ser::SerializeMap for &'a mut PhpSerializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut PhpSerializer::new(&mut self.output))
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut PhpSerializer::new(&mut self.output))
    }

    fn end(self) -> Result<()> {
        self.output.write_all(b"}").map_err(Error::WriteSerialized)
    }
}

impl<'a, W> ser::SerializeStruct for &'a mut PhpSerializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let mut ser = PhpSerializer::new(&mut self.output);
        key.serialize(&mut ser)?;
        value.serialize(&mut ser)?;
        Ok(())
    }

    fn end(self) -> Result<()> {
        self.output.write_all(b"}").map_err(Error::WriteSerialized)
    }
}

impl<'a, W> ser::SerializeStructVariant for &'a mut PhpSerializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::MissingFeature(
            "Serialization of enums is not supported. If you need C-style enums serialized, look at `serde_repr`.",
        ))
    }

    fn end(self) -> Result<()> {
        Err(Error::MissingFeature(
            "Serialization of enums is not supported. If you need C-style enums serialized, look at `serde_repr`.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::to_vec;
    use serde::Serialize;
    use std::collections::BTreeMap;

    macro_rules! assert_serializes {
        ($v:expr, $expected:expr) => {
            let actual = to_vec(&$v).expect("serialization failed");

            eprintln!("{}", String::from_utf8_lossy(actual.as_slice()));
            eprintln!("{}", String::from_utf8_lossy($expected));

            assert_eq!(actual.as_slice(), &$expected[..]);
        };
    }

    #[test]
    fn serialize_unit() {
        assert_serializes!((), b"N;");
    }

    #[test]
    fn serialize_bool() {
        assert_serializes!(false, b"b:0;");
        assert_serializes!(true, b"b:1;");
    }

    #[test]
    fn serialize_integer() {
        assert_serializes!(-1i64, b"i:-1;");
        assert_serializes!(0i64, b"i:0;");
        assert_serializes!(1i64, b"i:1;");
        assert_serializes!(123i64, b"i:123;");
    }

    #[test]
    fn serialize_float() {
        assert_serializes!(-1f64, b"d:-1;");
        assert_serializes!(0f64, b"d:0;");
        assert_serializes!(1f64, b"d:1;");
        assert_serializes!(-1.9f64, b"d:-1.9;");
        assert_serializes!(0.9f64, b"d:0.9;");
        assert_serializes!(1.9f64, b"d:1.9;");
    }

    #[test]
    fn serialize_php_string() {
        assert_serializes!(
            serde_bytes::Bytes::new(b"single quote '"),
            br#"s:14:"single quote '";"#
        );

        assert_serializes!(
            serde_bytes::ByteBuf::from(b"single quote '".to_vec()),
            br#"s:14:"single quote '";"#
        );
    }

    #[test]
    fn serialize_string() {
        assert_serializes!("single quote '", br#"s:14:"single quote '";"#);
        assert_serializes!("single quote '".to_owned(), br#"s:14:"single quote '";"#);
    }

    #[test]
    fn serialize_array() {
        #[derive(Debug, Serialize, Eq, PartialEq)]
        struct SubData();

        #[derive(Debug, Serialize, Eq, PartialEq)]
        struct Data(
            #[serde(with = "serde_bytes")] Vec<u8>,
            #[serde(with = "serde_bytes")] Vec<u8>,
            SubData,
        );

        assert_serializes!(
            Data(b"user".to_vec(), b"".to_vec(), SubData()),
            br#"a:3:{i:0;s:4:"user";i:1;s:0:"";i:2;a:0:{}}"#
        );
    }

    #[test]
    fn serialize_struct() {
        // PHP equiv:
        //
        // array("foo" => true,
        //       "bar" => "xyz",
        //       "sub" => array("x" => 42))

        #[derive(Debug, Serialize, Eq, PartialEq)]
        struct Outer {
            foo: bool,
            bar: String,
            sub: Inner,
        }

        #[derive(Debug, Serialize, Eq, PartialEq)]
        struct Inner {
            x: i64,
        }

        assert_serializes!(
            Outer {
                foo: true,
                bar: "xyz".to_owned(),
                sub: Inner { x: 42 },
            },
            br#"a:3:{s:3:"foo";b:1;s:3:"bar";s:3:"xyz";s:3:"sub";a:1:{s:1:"x";i:42;}}"#
        );
    }

    #[test]
    fn serialize_struct_with_optional() {
        #[derive(Debug, Serialize, Eq, PartialEq)]
        struct Location {
            #[serde(skip_serializing_if = "Option::is_none")]
            province: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            postalcode: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            country: Option<String>,
        }

        assert_serializes!(
            Location {
                province: None,
                postalcode: None,
                country: None,
            },
            br#"a:0:{}"#
        );

        assert_serializes!(
            Location {
                province: Some("Newfoundland and Labrador, CA".to_owned()),
                postalcode: None,
                country: None,
            },
            br#"a:1:{s:8:"province";s:29:"Newfoundland and Labrador, CA";}"#
        );

        assert_serializes!(
            Location {
                province: None,
                postalcode: Some("90002".to_owned()),
                country: Some("United States of America".to_owned()),
            },
            br#"a:2:{s:10:"postalcode";s:5:"90002";s:7:"country";s:24:"United States of America";}"#
        );
    }

    #[test]
    fn serialize_nested() {
        // PHP: array("x" => array("inner" => 1), "y" => array("inner" => 2))

        #[derive(Debug, Serialize, Eq, PartialEq)]
        struct Outer {
            x: Inner,
            y: Inner,
        }

        #[derive(Debug, Serialize, Eq, PartialEq)]
        struct Inner {
            inner: u8,
        }

        assert_serializes!(
            Outer {
                x: Inner { inner: 1 },
                y: Inner { inner: 2 },
            },
            br#"a:2:{s:1:"x";a:1:{s:5:"inner";i:1;}s:1:"y";a:1:{s:5:"inner";i:2;}}"#
        );
    }

    #[test]
    fn serialize_variable_length() {
        // PHP: array(1.1, 2.2, 3.3, 4.4)
        assert_serializes!(
            vec![1.1, 2.2, 3.3, 4.4],
            br#"a:4:{i:0;d:1.1;i:1;d:2.2;i:2;d:3.3;i:3;d:4.4;}"#
        );
    }

    #[test]
    fn serialize_btreemap() {
        // PHP: array("foo" => 1, "bar" => 2)
        let mut input: BTreeMap<String, u16> = BTreeMap::new();
        input.insert("foo".to_owned(), 42);
        input.insert("bar".to_owned(), 7);

        assert_serializes!(input, br#"a:2:{s:3:"bar";i:7;s:3:"foo";i:42;}"#);
    }
}
