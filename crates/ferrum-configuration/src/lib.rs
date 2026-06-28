//! Codecs for Minecraft Java Edition Configuration-state packet payloads.
//!
//! Packet IDs are deliberately not stored here. They belong to
//! `ferrum-protocol::ProtocolProfile`; this crate only produces deterministic
//! packet bodies and bounded decoders for client responses.

use ferrum_nbt::{NbtError, Tag, encode_anonymous};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KnownPack {
    pub namespace: String,
    pub id: String,
    pub version: String,
}

impl KnownPack {
    #[must_use]
    pub fn new(
        namespace: impl Into<String>,
        id: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            id: id.into(),
            version: version.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownPackDecodeLimits {
    pub max_packs: usize,
    pub max_string_bytes: usize,
}

impl Default for KnownPackDecodeLimits {
    fn default() -> Self {
        Self {
            max_packs: 64,
            max_string_bytes: 32_767,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegistryEntry {
    pub key: String,
    pub value: Option<Tag>,
}

impl RegistryEntry {
    #[must_use]
    pub fn new(key: impl Into<String>, value: Option<Tag>) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegistryData {
    pub id: String,
    pub entries: Vec<RegistryEntry>,
}

impl RegistryData {
    #[must_use]
    pub fn new(id: impl Into<String>, entries: Vec<RegistryEntry>) -> Self {
        Self {
            id: id.into(),
            entries,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagEntry {
    pub name: String,
    pub entries: Vec<i32>,
}

impl TagEntry {
    #[must_use]
    pub fn new(name: impl Into<String>, entries: Vec<i32>) -> Self {
        Self {
            name: name.into(),
            entries,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRegistry {
    pub id: String,
    pub tags: Vec<TagEntry>,
}

impl TagRegistry {
    #[must_use]
    pub fn new(id: impl Into<String>, tags: Vec<TagEntry>) -> Self {
        Self {
            id: id.into(),
            tags,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigurationEncodeError {
    #[error("resource location cannot be empty")]
    EmptyResourceLocation,
    #[error("known-pack {field} cannot be empty")]
    EmptyKnownPackField { field: &'static str },
    #[error("collection length {length} exceeds the protocol VarInt range")]
    CollectionTooLong { length: usize },
    #[error("string length {length} exceeds the protocol VarInt range")]
    StringTooLong { length: usize },
    #[error("negative registry entry ID {0} is not valid in a tag")]
    NegativeRegistryEntry(i32),
    #[error("cannot encode registry NBT: {0}")]
    Nbt(#[from] NbtError),
}

#[derive(Debug, Error)]
pub enum ConfigurationDecodeError {
    #[error("Configuration payload ended unexpectedly")]
    UnexpectedEnd,
    #[error("Configuration VarInt exceeds five bytes")]
    VarIntTooLong,
    #[error("negative {what} length {length}")]
    NegativeLength { what: &'static str, length: i32 },
    #[error("{what} exceeds configured limit {limit}")]
    LimitExceeded { what: &'static str, limit: usize },
    #[error("Configuration string is not valid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
    #[error("Configuration payload contains {0} trailing bytes")]
    TrailingBytes(usize),
}

pub fn encode_known_packs(packs: &[KnownPack]) -> Result<Vec<u8>, ConfigurationEncodeError> {
    let mut output = Vec::new();
    write_len(&mut output, packs.len())?;
    for pack in packs {
        validate_known_pack_field("namespace", &pack.namespace)?;
        validate_known_pack_field("id", &pack.id)?;
        validate_known_pack_field("version", &pack.version)?;
        write_string(&mut output, &pack.namespace)?;
        write_string(&mut output, &pack.id)?;
        write_string(&mut output, &pack.version)?;
    }
    Ok(output)
}

pub fn decode_known_packs(
    payload: &[u8],
    limits: KnownPackDecodeLimits,
) -> Result<Vec<KnownPack>, ConfigurationDecodeError> {
    let mut decoder = Decoder::new(payload);
    let count = decoder.read_non_negative_length("known-pack array")?;
    if count > limits.max_packs {
        return Err(ConfigurationDecodeError::LimitExceeded {
            what: "known-pack count",
            limit: limits.max_packs,
        });
    }

    let mut packs = Vec::with_capacity(count);
    for _ in 0..count {
        packs.push(KnownPack::new(
            decoder.read_string(limits.max_string_bytes)?,
            decoder.read_string(limits.max_string_bytes)?,
            decoder.read_string(limits.max_string_bytes)?,
        ));
    }
    decoder.finish()?;
    Ok(packs)
}

pub fn encode_registry_data(registry: &RegistryData) -> Result<Vec<u8>, ConfigurationEncodeError> {
    validate_resource_location(&registry.id)?;
    let mut output = Vec::new();
    write_string(&mut output, &registry.id)?;
    write_len(&mut output, registry.entries.len())?;
    for entry in &registry.entries {
        validate_resource_location(&entry.key)?;
        write_string(&mut output, &entry.key)?;
        match &entry.value {
            Some(value) => {
                output.push(1);
                encode_anonymous(&mut output, value)?;
            }
            None => output.push(0),
        }
    }
    Ok(output)
}

pub fn encode_feature_flags(features: &[String]) -> Result<Vec<u8>, ConfigurationEncodeError> {
    let mut output = Vec::new();
    write_len(&mut output, features.len())?;
    for feature in features {
        validate_resource_location(feature)?;
        write_string(&mut output, feature)?;
    }
    Ok(output)
}

pub fn encode_tags(registries: &[TagRegistry]) -> Result<Vec<u8>, ConfigurationEncodeError> {
    let mut output = Vec::new();
    write_len(&mut output, registries.len())?;
    for registry in registries {
        validate_resource_location(&registry.id)?;
        write_string(&mut output, &registry.id)?;
        write_len(&mut output, registry.tags.len())?;
        for tag in &registry.tags {
            validate_resource_location(&tag.name)?;
            write_string(&mut output, &tag.name)?;
            write_len(&mut output, tag.entries.len())?;
            for entry in &tag.entries {
                if *entry < 0 {
                    return Err(ConfigurationEncodeError::NegativeRegistryEntry(*entry));
                }
                write_varint(&mut output, *entry);
            }
        }
    }
    Ok(output)
}

fn validate_resource_location(value: &str) -> Result<(), ConfigurationEncodeError> {
    if value.is_empty() {
        return Err(ConfigurationEncodeError::EmptyResourceLocation);
    }
    Ok(())
}

fn validate_known_pack_field(
    field: &'static str,
    value: &str,
) -> Result<(), ConfigurationEncodeError> {
    if value.is_empty() {
        return Err(ConfigurationEncodeError::EmptyKnownPackField { field });
    }
    Ok(())
}

fn write_string(output: &mut Vec<u8>, value: &str) -> Result<(), ConfigurationEncodeError> {
    let length =
        i32::try_from(value.len()).map_err(|_| ConfigurationEncodeError::StringTooLong {
            length: value.len(),
        })?;
    write_varint(output, length);
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_len(output: &mut Vec<u8>, length: usize) -> Result<(), ConfigurationEncodeError> {
    let length = i32::try_from(length)
        .map_err(|_| ConfigurationEncodeError::CollectionTooLong { length })?;
    write_varint(output, length);
    Ok(())
}

fn write_varint(output: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            return;
        }
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn read_varint(&mut self) -> Result<i32, ConfigurationDecodeError> {
        let mut value = 0i32;
        for position in 0..5 {
            let byte = self.read_u8()?;
            value |= i32::from(byte & 0x7f) << (7 * position);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(ConfigurationDecodeError::VarIntTooLong)
    }

    fn read_non_negative_length(
        &mut self,
        what: &'static str,
    ) -> Result<usize, ConfigurationDecodeError> {
        let length = self.read_varint()?;
        if length < 0 {
            return Err(ConfigurationDecodeError::NegativeLength { what, length });
        }
        Ok(length as usize)
    }

    fn read_string(&mut self, max_bytes: usize) -> Result<String, ConfigurationDecodeError> {
        let length = self.read_non_negative_length("string")?;
        if length > max_bytes {
            return Err(ConfigurationDecodeError::LimitExceeded {
                what: "string length",
                limit: max_bytes,
            });
        }
        Ok(String::from_utf8(self.read_bytes(length)?.to_vec())?)
    }

    fn read_u8(&mut self) -> Result<u8, ConfigurationDecodeError> {
        Ok(*self
            .read_bytes(1)?
            .first()
            .expect("one byte was just read"))
    }

    fn read_bytes(&mut self, length: usize) -> Result<&'a [u8], ConfigurationDecodeError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(ConfigurationDecodeError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.cursor..end)
            .ok_or(ConfigurationDecodeError::UnexpectedEnd)?;
        self.cursor = end;
        Ok(bytes)
    }

    fn finish(self) -> Result<(), ConfigurationDecodeError> {
        let remaining = self.bytes.len().saturating_sub(self.cursor);
        if remaining == 0 {
            Ok(())
        } else {
            Err(ConfigurationDecodeError::TrailingBytes(remaining))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn known_packs_round_trip() {
        let packs = vec![KnownPack::new("minecraft", "core", "26.1.2")];
        let encoded = encode_known_packs(&packs).unwrap();
        assert_eq!(
            encoded,
            vec![
                1, 9, b'm', b'i', b'n', b'e', b'c', b'r', b'a', b'f', b't', 4, b'c', b'o', b'r',
                b'e', 6, b'2', b'6', b'.', b'1', b'.', b'2',
            ]
        );
        assert_eq!(
            decode_known_packs(&encoded, KnownPackDecodeLimits::default()).unwrap(),
            packs
        );
    }

    #[test]
    fn known_pack_decoder_enforces_count_limit() {
        let error = decode_known_packs(
            &[1, 0, 0, 0],
            KnownPackDecodeLimits {
                max_packs: 0,
                ..KnownPackDecodeLimits::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ConfigurationDecodeError::LimitExceeded {
                what: "known-pack count",
                limit: 0,
            }
        ));
    }

    #[test]
    fn known_pack_decoder_rejects_trailing_bytes() {
        let mut encoded = encode_known_packs(&[]).unwrap();
        encoded.push(0);
        assert!(matches!(
            decode_known_packs(&encoded, KnownPackDecodeLimits::default()),
            Err(ConfigurationDecodeError::TrailingBytes(1))
        ));
    }

    #[test]
    fn encodes_registry_data_with_optional_anonymous_nbt() {
        let mut value = BTreeMap::new();
        value.insert("natural".to_owned(), Tag::Byte(1));
        let registry = RegistryData::new(
            "minecraft:dimension_type",
            vec![
                RegistryEntry::new("minecraft:overworld", Some(Tag::Compound(value))),
                RegistryEntry::new("minecraft:the_nether", None),
            ],
        );

        let encoded = encode_registry_data(&registry).unwrap();
        let mut expected = Vec::new();
        write_string(&mut expected, "minecraft:dimension_type").unwrap();
        write_varint(&mut expected, 2);
        write_string(&mut expected, "minecraft:overworld").unwrap();
        expected.push(1);
        expected.extend_from_slice(&[
            10, // anonymous TAG_Compound root
            1, 0, 7, b'n', b'a', b't', b'u', b'r', b'a', b'l', 1, 0,
        ]);
        write_string(&mut expected, "minecraft:the_nether").unwrap();
        expected.push(0);
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encodes_feature_flags_as_string_array() {
        assert_eq!(
            encode_feature_flags(&["minecraft:vanilla".to_owned()]).unwrap(),
            vec![
                1, 17, b'm', b'i', b'n', b'e', b'c', b'r', b'a', b'f', b't', b':', b'v', b'a',
                b'n', b'i', b'l', b'l', b'a',
            ]
        );
    }

    #[test]
    fn encodes_empty_tag_registry_list() {
        assert_eq!(encode_tags(&[]).unwrap(), vec![0]);
    }

    #[test]
    fn rejects_negative_tag_entry_ids() {
        let error = encode_tags(&[TagRegistry::new(
            "minecraft:block",
            vec![TagEntry::new("minecraft:test", vec![-1])],
        )])
        .unwrap_err();
        assert!(matches!(
            error,
            ConfigurationEncodeError::NegativeRegistryEntry(-1)
        ));
    }
}
