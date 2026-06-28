//! Deterministic binary Named Binary Tag (NBT) codec used by Ferrum.
//!
//! The codec intentionally keeps its data model small and explicit. Compound
//! values use [`BTreeMap`] so encoded output is stable across runs, which is
//! useful for fixtures, protocol tests, and differential testing.

use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
};

use thiserror::Error;

/// The numeric tag identifiers defined by the binary NBT format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum TagType {
    End = 0,
    Byte = 1,
    Short = 2,
    Int = 3,
    Long = 4,
    Float = 5,
    Double = 6,
    ByteArray = 7,
    String = 8,
    List = 9,
    Compound = 10,
    IntArray = 11,
    LongArray = 12,
}

impl TryFrom<u8> for TagType {
    type Error = NbtError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::End),
            1 => Ok(Self::Byte),
            2 => Ok(Self::Short),
            3 => Ok(Self::Int),
            4 => Ok(Self::Long),
            5 => Ok(Self::Float),
            6 => Ok(Self::Double),
            7 => Ok(Self::ByteArray),
            8 => Ok(Self::String),
            9 => Ok(Self::List),
            10 => Ok(Self::Compound),
            11 => Ok(Self::IntArray),
            12 => Ok(Self::LongArray),
            other => Err(NbtError::InvalidTagType(other)),
        }
    }
}

/// One NBT payload.
#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List {
        element_type: TagType,
        elements: Vec<Tag>,
    },
    Compound(BTreeMap<String, Tag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    #[must_use]
    pub const fn tag_type(&self) -> TagType {
        match self {
            Self::End => TagType::End,
            Self::Byte(_) => TagType::Byte,
            Self::Short(_) => TagType::Short,
            Self::Int(_) => TagType::Int,
            Self::Long(_) => TagType::Long,
            Self::Float(_) => TagType::Float,
            Self::Double(_) => TagType::Double,
            Self::ByteArray(_) => TagType::ByteArray,
            Self::String(_) => TagType::String,
            Self::List { .. } => TagType::List,
            Self::Compound(_) => TagType::Compound,
            Self::IntArray(_) => TagType::IntArray,
            Self::LongArray(_) => TagType::LongArray,
        }
    }
}

/// A root NBT value and its root name.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedTag {
    pub name: String,
    pub tag: Tag,
}

impl NamedTag {
    #[must_use]
    pub fn new(name: impl Into<String>, tag: Tag) -> Self {
        Self {
            name: name.into(),
            tag,
        }
    }

    #[must_use]
    pub fn unnamed(tag: Tag) -> Self {
        Self::new(String::new(), tag)
    }
}

/// Resource limits applied while decoding untrusted NBT input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    pub max_depth: usize,
    pub max_string_bytes: usize,
    pub max_collection_len: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_depth: 512,
            max_string_bytes: 1 << 20,
            max_collection_len: 1_000_000,
        }
    }
}

#[derive(Debug, Error)]
pub enum NbtError {
    #[error("I/O error while processing NBT: {0}")]
    Io(#[from] io::Error),
    #[error("unexpected end of NBT input")]
    UnexpectedEnd,
    #[error("invalid NBT tag type {0}")]
    InvalidTagType(u8),
    #[error("TAG_End cannot be used as a named root or compound value")]
    InvalidNamedEnd,
    #[error("negative {kind} length {length}")]
    NegativeLength { kind: &'static str, length: i32 },
    #[error("{what} exceeds configured limit {limit}")]
    LimitExceeded { what: &'static str, limit: usize },
    #[error("NBT string length {length} exceeds the u16 format limit")]
    StringTooLong { length: usize },
    #[error("NBT collection length {length} exceeds the i32 format limit")]
    CollectionTooLong { length: usize },
    #[error("invalid UTF-8 in NBT string: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
    #[error("heterogeneous TAG_List at index {index}: expected {expected:?}, found {found:?}")]
    HeterogeneousList {
        expected: TagType,
        found: TagType,
        index: usize,
    },
    #[error("non-empty TAG_List cannot use TAG_End as its element type")]
    EndTypedNonEmptyList,
    #[error("NBT nesting depth exceeds configured limit {limit}")]
    DepthLimitExceeded { limit: usize },
}

/// Encode a named NBT root.
pub fn encode_named<W: Write>(mut writer: W, root: &NamedTag) -> Result<(), NbtError> {
    let tag_type = root.tag.tag_type();
    if tag_type == TagType::End {
        return Err(NbtError::InvalidNamedEnd);
    }
    write_u8(&mut writer, tag_type as u8)?;
    write_string(&mut writer, &root.name)?;
    write_payload(&mut writer, &root.tag)
}

/// Encode a standard named-root value using an empty root name.
pub fn encode_unnamed<W: Write>(writer: W, tag: &Tag) -> Result<(), NbtError> {
    encode_named(writer, &NamedTag::unnamed(tag.clone()))
}

/// Encode protocol anonymous NBT: a root tag type followed directly by its payload.
pub fn encode_anonymous<W: Write>(mut writer: W, tag: &Tag) -> Result<(), NbtError> {
    let tag_type = tag.tag_type();
    if tag_type == TagType::End {
        return Err(NbtError::InvalidNamedEnd);
    }
    write_u8(&mut writer, tag_type as u8)?;
    write_payload(&mut writer, tag)
}

/// Decode a named NBT root using conservative default limits.
pub fn decode_named<R: Read>(reader: R) -> Result<NamedTag, NbtError> {
    decode_named_with_limits(reader, DecodeLimits::default())
}

/// Decode a named NBT root using caller-provided resource limits.
pub fn decode_named_with_limits<R: Read>(
    mut reader: R,
    limits: DecodeLimits,
) -> Result<NamedTag, NbtError> {
    let tag_type = TagType::try_from(read_u8(&mut reader)?)?;
    if tag_type == TagType::End {
        return Err(NbtError::InvalidNamedEnd);
    }
    let name = read_string(&mut reader, limits)?;
    let tag = read_payload(&mut reader, tag_type, limits, 0)?;
    Ok(NamedTag { name, tag })
}

/// Decode protocol anonymous NBT using conservative default limits.
pub fn decode_anonymous<R: Read>(reader: R) -> Result<Tag, NbtError> {
    decode_anonymous_with_limits(reader, DecodeLimits::default())
}

/// Decode protocol anonymous NBT using caller-provided resource limits.
pub fn decode_anonymous_with_limits<R: Read>(
    mut reader: R,
    limits: DecodeLimits,
) -> Result<Tag, NbtError> {
    let tag_type = TagType::try_from(read_u8(&mut reader)?)?;
    if tag_type == TagType::End {
        return Err(NbtError::InvalidNamedEnd);
    }
    read_payload(&mut reader, tag_type, limits, 0)
}

fn write_payload<W: Write>(writer: &mut W, tag: &Tag) -> Result<(), NbtError> {
    match tag {
        Tag::End => Err(NbtError::InvalidNamedEnd),
        Tag::Byte(value) => write_u8(writer, *value as u8),
        Tag::Short(value) => write_all(writer, &value.to_be_bytes()),
        Tag::Int(value) => write_all(writer, &value.to_be_bytes()),
        Tag::Long(value) => write_all(writer, &value.to_be_bytes()),
        Tag::Float(value) => write_all(writer, &value.to_bits().to_be_bytes()),
        Tag::Double(value) => write_all(writer, &value.to_bits().to_be_bytes()),
        Tag::ByteArray(values) => {
            write_len(writer, values.len())?;
            let bytes: Vec<u8> = values.iter().map(|value| *value as u8).collect();
            write_all(writer, &bytes)
        }
        Tag::String(value) => write_string(writer, value),
        Tag::List {
            element_type,
            elements,
        } => {
            if *element_type == TagType::End && !elements.is_empty() {
                return Err(NbtError::EndTypedNonEmptyList);
            }
            for (index, element) in elements.iter().enumerate() {
                let found = element.tag_type();
                if found != *element_type {
                    return Err(NbtError::HeterogeneousList {
                        expected: *element_type,
                        found,
                        index,
                    });
                }
            }
            write_u8(writer, *element_type as u8)?;
            write_len(writer, elements.len())?;
            for element in elements {
                write_payload(writer, element)?;
            }
            Ok(())
        }
        Tag::Compound(values) => {
            for (name, value) in values {
                let tag_type = value.tag_type();
                if tag_type == TagType::End {
                    return Err(NbtError::InvalidNamedEnd);
                }
                write_u8(writer, tag_type as u8)?;
                write_string(writer, name)?;
                write_payload(writer, value)?;
            }
            write_u8(writer, TagType::End as u8)
        }
        Tag::IntArray(values) => {
            write_len(writer, values.len())?;
            for value in values {
                write_all(writer, &value.to_be_bytes())?;
            }
            Ok(())
        }
        Tag::LongArray(values) => {
            write_len(writer, values.len())?;
            for value in values {
                write_all(writer, &value.to_be_bytes())?;
            }
            Ok(())
        }
    }
}

fn read_payload<R: Read>(
    reader: &mut R,
    tag_type: TagType,
    limits: DecodeLimits,
    depth: usize,
) -> Result<Tag, NbtError> {
    if depth > limits.max_depth {
        return Err(NbtError::DepthLimitExceeded {
            limit: limits.max_depth,
        });
    }

    match tag_type {
        TagType::End => Ok(Tag::End),
        TagType::Byte => Ok(Tag::Byte(read_u8(reader)? as i8)),
        TagType::Short => Ok(Tag::Short(i16::from_be_bytes(read_array(reader)?))),
        TagType::Int => Ok(Tag::Int(i32::from_be_bytes(read_array(reader)?))),
        TagType::Long => Ok(Tag::Long(i64::from_be_bytes(read_array(reader)?))),
        TagType::Float => Ok(Tag::Float(f32::from_bits(u32::from_be_bytes(read_array(
            reader,
        )?)))),
        TagType::Double => Ok(Tag::Double(f64::from_bits(u64::from_be_bytes(read_array(
            reader,
        )?)))),
        TagType::ByteArray => {
            let length = read_len(reader, "byte array", limits.max_collection_len)?;
            let mut bytes = vec![0; length];
            read_exact(reader, &mut bytes)?;
            Ok(Tag::ByteArray(
                bytes.into_iter().map(|value| value as i8).collect(),
            ))
        }
        TagType::String => Ok(Tag::String(read_string(reader, limits)?)),
        TagType::List => {
            let element_type = TagType::try_from(read_u8(reader)?)?;
            let length = read_len(reader, "list", limits.max_collection_len)?;
            if element_type == TagType::End && length != 0 {
                return Err(NbtError::EndTypedNonEmptyList);
            }
            let mut elements = Vec::with_capacity(length);
            for _ in 0..length {
                elements.push(read_payload(reader, element_type, limits, depth + 1)?);
            }
            Ok(Tag::List {
                element_type,
                elements,
            })
        }
        TagType::Compound => {
            let mut values = BTreeMap::new();
            loop {
                let child_type = TagType::try_from(read_u8(reader)?)?;
                if child_type == TagType::End {
                    break;
                }
                let name = read_string(reader, limits)?;
                let value = read_payload(reader, child_type, limits, depth + 1)?;
                values.insert(name, value);
            }
            Ok(Tag::Compound(values))
        }
        TagType::IntArray => {
            let length = read_len(reader, "int array", limits.max_collection_len)?;
            let mut values = Vec::with_capacity(length);
            for _ in 0..length {
                values.push(i32::from_be_bytes(read_array(reader)?));
            }
            Ok(Tag::IntArray(values))
        }
        TagType::LongArray => {
            let length = read_len(reader, "long array", limits.max_collection_len)?;
            let mut values = Vec::with_capacity(length);
            for _ in 0..length {
                values.push(i64::from_be_bytes(read_array(reader)?));
            }
            Ok(Tag::LongArray(values))
        }
    }
}

fn write_string<W: Write>(writer: &mut W, value: &str) -> Result<(), NbtError> {
    let length = value.len();
    let encoded_length = u16::try_from(length).map_err(|_| NbtError::StringTooLong { length })?;
    write_all(writer, &encoded_length.to_be_bytes())?;
    write_all(writer, value.as_bytes())
}

fn read_string<R: Read>(reader: &mut R, limits: DecodeLimits) -> Result<String, NbtError> {
    let length = u16::from_be_bytes(read_array(reader)?) as usize;
    if length > limits.max_string_bytes {
        return Err(NbtError::LimitExceeded {
            what: "string length",
            limit: limits.max_string_bytes,
        });
    }
    let mut bytes = vec![0; length];
    read_exact(reader, &mut bytes)?;
    Ok(String::from_utf8(bytes)?)
}

fn write_len<W: Write>(writer: &mut W, length: usize) -> Result<(), NbtError> {
    let encoded = i32::try_from(length).map_err(|_| NbtError::CollectionTooLong { length })?;
    write_all(writer, &encoded.to_be_bytes())
}

fn read_len<R: Read>(reader: &mut R, kind: &'static str, limit: usize) -> Result<usize, NbtError> {
    let length = i32::from_be_bytes(read_array(reader)?);
    if length < 0 {
        return Err(NbtError::NegativeLength { kind, length });
    }
    let length = length as usize;
    if length > limit {
        return Err(NbtError::LimitExceeded { what: kind, limit });
    }
    Ok(length)
}

fn read_u8<R: Read>(reader: &mut R) -> Result<u8, NbtError> {
    Ok(read_array::<1, _>(reader)?[0])
}

fn write_u8<W: Write>(writer: &mut W, value: u8) -> Result<(), NbtError> {
    write_all(writer, &[value])
}

fn read_array<const N: usize, R: Read>(reader: &mut R) -> Result<[u8; N], NbtError> {
    let mut bytes = [0; N];
    read_exact(reader, &mut bytes)?;
    Ok(bytes)
}

fn read_exact<R: Read>(reader: &mut R, bytes: &mut [u8]) -> Result<(), NbtError> {
    match reader.read_exact(bytes) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Err(NbtError::UnexpectedEnd),
        Err(error) => Err(NbtError::Io(error)),
    }
}

fn write_all<W: Write>(writer: &mut W, bytes: &[u8]) -> Result<(), NbtError> {
    writer.write_all(bytes).map_err(NbtError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trips_nested_compound() {
        let mut player = BTreeMap::new();
        player.insert("health".to_owned(), Tag::Int(20));
        player.insert("name".to_owned(), Tag::String("Steve".to_owned()));
        player.insert(
            "position".to_owned(),
            Tag::List {
                element_type: TagType::Double,
                elements: vec![Tag::Double(1.5), Tag::Double(64.0), Tag::Double(-3.25)],
            },
        );
        player.insert("flags".to_owned(), Tag::ByteArray(vec![-1, 0, 127]));
        player.insert("scores".to_owned(), Tag::IntArray(vec![1, 2, 3]));
        player.insert("timestamps".to_owned(), Tag::LongArray(vec![4, 5, 6]));

        let root = NamedTag::new("Player", Tag::Compound(player));
        let mut encoded = Vec::new();
        encode_named(&mut encoded, &root).expect("NBT should encode");
        let decoded = decode_named(Cursor::new(encoded)).expect("NBT should decode");
        assert_eq!(decoded, root);
    }

    #[test]
    fn encodes_compound_in_deterministic_key_order() {
        let mut values = BTreeMap::new();
        values.insert("z".to_owned(), Tag::Byte(2));
        values.insert("a".to_owned(), Tag::Byte(1));
        let root = NamedTag::unnamed(Tag::Compound(values));

        let mut encoded = Vec::new();
        encode_named(&mut encoded, &root).unwrap();

        assert_eq!(
            encoded,
            vec![
                10, 0, 0, // unnamed compound root
                1, 0, 1, b'a', 1, // a = 1
                1, 0, 1, b'z', 2, // z = 2
                0, // compound terminator
            ]
        );
    }

    #[test]
    fn encodes_known_int_compound_bytes() {
        let mut values = BTreeMap::new();
        values.insert("health".to_owned(), Tag::Int(20));
        let root = NamedTag::unnamed(Tag::Compound(values));
        let mut encoded = Vec::new();
        encode_named(&mut encoded, &root).unwrap();

        assert_eq!(
            encoded,
            vec![
                10, 0, 0, // unnamed compound root
                3, 0, 6, b'h', b'e', b'a', b'l', b't', b'h', 0, 0, 0, 20, 0,
            ]
        );
    }

    #[test]
    fn rejects_heterogeneous_lists() {
        let root = NamedTag::unnamed(Tag::List {
            element_type: TagType::Int,
            elements: vec![Tag::Int(1), Tag::String("wrong".to_owned())],
        });
        let error = encode_named(Vec::new(), &root).unwrap_err();
        assert!(matches!(
            error,
            NbtError::HeterogeneousList {
                expected: TagType::Int,
                found: TagType::String,
                index: 1,
            }
        ));
    }

    #[test]
    fn rejects_negative_collection_lengths() {
        let bytes = [
            TagType::List as u8,
            0,
            0, // unnamed root
            TagType::Int as u8,
            0xff,
            0xff,
            0xff,
            0xff, // -1 elements
        ];
        let error = decode_named(Cursor::new(bytes)).unwrap_err();
        assert!(matches!(
            error,
            NbtError::NegativeLength {
                kind: "list",
                length: -1,
            }
        ));
    }

    #[test]
    fn enforces_depth_limit() {
        let mut child = BTreeMap::new();
        child.insert("value".to_owned(), Tag::Int(1));
        let mut root_values = BTreeMap::new();
        root_values.insert("child".to_owned(), Tag::Compound(child));
        let root = NamedTag::unnamed(Tag::Compound(root_values));

        let mut encoded = Vec::new();
        encode_named(&mut encoded, &root).unwrap();
        let limits = DecodeLimits {
            max_depth: 0,
            ..DecodeLimits::default()
        };
        let error = decode_named_with_limits(Cursor::new(encoded), limits).unwrap_err();
        assert!(matches!(error, NbtError::DepthLimitExceeded { limit: 0 }));
    }

    #[test]
    fn anonymous_nbt_omits_the_root_name() {
        let mut values = BTreeMap::new();
        values.insert("value".to_owned(), Tag::Int(7));
        let tag = Tag::Compound(values);
        let mut encoded = Vec::new();
        encode_anonymous(&mut encoded, &tag).unwrap();
        assert_eq!(
            encoded,
            vec![10, 3, 0, 5, b'v', b'a', b'l', b'u', b'e', 0, 0, 0, 7, 0]
        );
        assert_eq!(decode_anonymous(Cursor::new(encoded)).unwrap(), tag);
    }

    #[test]
    fn rejects_truncated_input() {
        let error = decode_named(Cursor::new([TagType::Int as u8, 0])).unwrap_err();
        assert!(matches!(error, NbtError::UnexpectedEnd));
    }
}
