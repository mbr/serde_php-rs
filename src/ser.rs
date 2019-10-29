use crate::error::Error;
use serde::{ser, Serialize};
use std::io::Write;

type Result<T> = ::core::result::Result<T, Error>;

#[derive(Debug)]
pub struct PhpSerializer<W> {
    output: W,
}

impl<W> PhpSerializer<W> {
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

    fn serialize_bool(self, v: bool) -> Result<()> {
        if v {
            self.output.write_all(b"b:1;")
        } else {
            self.output.write_all(b"b:0;")
        }
        .map_err(Error::WriteSerialized)
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        // We rely on Rust having a "standard" display implementation for
        // `i64` types, which is a reasonable assumption.
        write!(self.output, "i:{};", v).map_err(Error::WriteSerialized)
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        write!(self.output, "i:{};", v).map_err(Error::WriteSerialized)
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.serialize_f64(f64::from(v))
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        // Float representations _should_ match up.
        // TODO: Verify this prints edges correctly.
        write!(self.output, "i:{};", v).map_err(Error::WriteSerialized)
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.serialize_bytes(v.as_bytes())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        write!(self.output, "s:{}:\"", v.len()).map_err(Error::WriteSerialized)?;
        self.output.write_all(v).map_err(Error::WriteSerialized)?;
        write!(self.output, "\"").map_err(Error::WriteSerialized)
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        self.output.write_all(b"N;").map_err(Error::WriteSerialized)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(Error::MissingFeature(
            "Serialization of unit structures is not supported.",
        ))
    }

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

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // We just "unpack" newtypes when deserializing.
        value.serialize(self)
    }

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

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_tuple(len)
    }

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

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        if let Some(n) = len {
            write!(self.output, "a:{}:{{", n).map_err(Error::WriteSerialized)?;
            // No need to count elements, thus no added state.
            Ok(self)
        } else {
            return Err(Error::LengthRequired);
        }
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

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

#[derive(Debug)]
pub struct NumericArraySerializer<'a, W> {
    // There is no delimiter for elements (arrays are length-prefixed and
    // and carry their own terminator. However, we still need to count
    // the elements.
    index: usize,
    serializer: &'a mut PhpSerializer<W>,
}

impl<'a, W> NumericArraySerializer<'a, W> {
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
        unimplemented!()
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
