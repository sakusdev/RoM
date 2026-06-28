//! Encoders for Minecraft Java Edition Configuration-state packet payloads.
//!
//! Packet IDs are deliberately not stored here. They belong to
//! `ferrum-protocol::ProtocolProfile`; this crate only produces deterministic
//! packet bodies.

use ferrum_nbt::{NbtError, Tag, encode_anonymous};
use thiserror::Error;

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
    #[error("collection length {length} exceeds the protocol VarInt range")]
    CollectionTooLong { length: usize },
    #[error("string length {length} exceeds the protocol VarInt range")]
    StringTooLong { length: usize },
    #[error("negative registry entry ID {0} is not valid in a tag")]
    NegativeRegistryEntry(i32),
    #[error("cannot encode registry NBT: {0}")]
    Nbt(#[from] NbtError),
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

fn write_string(
    output: &mut Vec<u8>,
    value: &str,
) -> Result<(), ConfigurationEncodeError> {
    let length = i32::try_from(value.len()).map_err(|_| ConfigurationEncodeError::StringTooLong {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

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
