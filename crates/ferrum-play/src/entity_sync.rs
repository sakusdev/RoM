use std::collections::BTreeMap;

use ferrum_game::{
    AttributeInstance, AttributeOperation, AttributeSet, DamageKind, DamageSource, EntityId,
    StatusEffectInstance, Velocity,
};
use thiserror::Error;

const MAX_SYNC_ATTRIBUTES: usize = 256;
const MAX_SYNC_MODIFIERS: usize = 1_024;
const MAX_IDENTIFIER_BYTES: usize = 32_767;
const VELOCITY_SCALE: f64 = 8_000.0;
const MAX_PACKET_VELOCITY: f64 = 3.9;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProtocolIdRegistry {
    ids: BTreeMap<String, i32>,
    names: BTreeMap<i32, String>,
}

impl ProtocolIdRegistry {
    pub fn new<I, S>(entries: I) -> Result<Self, EntitySyncEncodeError>
    where
        I: IntoIterator<Item = (S, i32)>,
        S: Into<String>,
    {
        let mut ids = BTreeMap::new();
        let mut names = BTreeMap::new();
        for (id, protocol_id) in entries {
            let id = id.into();
            validate_identifier(&id)?;
            if protocol_id < 0 {
                return Err(EntitySyncEncodeError::NegativeProtocolId { id, protocol_id });
            }
            if ids.insert(id.clone(), protocol_id).is_some() {
                return Err(EntitySyncEncodeError::DuplicateRegistryEntry { id });
            }
            if let Some(previous) = names.insert(protocol_id, id.clone()) {
                return Err(EntitySyncEncodeError::DuplicateProtocolId {
                    protocol_id,
                    first: previous,
                    second: id,
                });
            }
        }
        Ok(Self { ids, names })
    }

    #[must_use]
    pub fn protocol_id(&self, id: &str) -> Option<i32> {
        self.ids.get(id).copied()
    }

    #[must_use]
    pub fn id(&self, protocol_id: i32) -> Option<&str> {
        self.names.get(&protocol_id).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

pub fn encode_update_attributes(
    entity_id: EntityId,
    attributes: &AttributeSet,
    registry: &ProtocolIdRegistry,
) -> Result<Option<Vec<u8>>, EntitySyncEncodeError> {
    if registry.is_empty() {
        return Ok(None);
    }
    let entries = attributes
        .iter()
        .map(|(id, instance)| (id.as_str(), instance))
        .collect::<Vec<_>>();
    encode_attribute_entries(entity_id, &entries, registry).map(Some)
}

pub fn encode_update_attribute(
    entity_id: EntityId,
    attribute: &str,
    instance: &AttributeInstance,
    registry: &ProtocolIdRegistry,
) -> Result<Option<Vec<u8>>, EntitySyncEncodeError> {
    if registry.is_empty() {
        return Ok(None);
    }
    encode_attribute_entries(entity_id, &[(attribute, instance)], registry).map(Some)
}

fn encode_attribute_entries(
    entity_id: EntityId,
    entries: &[(&str, &AttributeInstance)],
    registry: &ProtocolIdRegistry,
) -> Result<Vec<u8>, EntitySyncEncodeError> {
    if entries.is_empty() || entries.len() > MAX_SYNC_ATTRIBUTES {
        return Err(EntitySyncEncodeError::AttributeCountOutOfRange {
            count: entries.len(),
        });
    }
    let mut output = Vec::new();
    write_entity_id(&mut output, entity_id)?;
    write_len(&mut output, entries.len())?;
    for (attribute, instance) in entries {
        let protocol_id = registry.protocol_id(attribute).ok_or_else(|| {
            EntitySyncEncodeError::MissingProtocolId {
                id: (*attribute).to_owned(),
            }
        })?;
        write_varint(&mut output, protocol_id);
        output.extend_from_slice(&instance.base().to_be_bytes());
        if instance.modifiers().len() > MAX_SYNC_MODIFIERS {
            return Err(EntitySyncEncodeError::ModifierCountOutOfRange {
                count: instance.modifiers().len(),
            });
        }
        write_len(&mut output, instance.modifiers().len())?;
        for modifier in instance.modifiers().values() {
            write_identifier(&mut output, &modifier.id)?;
            output.extend_from_slice(&modifier.amount.to_be_bytes());
            output.push(match modifier.operation {
                AttributeOperation::AddValue => 0,
                AttributeOperation::AddMultipliedBase => 1,
                AttributeOperation::AddMultipliedTotal => 2,
            });
        }
    }
    Ok(output)
}

pub fn encode_update_mob_effect(
    entity_id: EntityId,
    effect: &StatusEffectInstance,
    registry: &ProtocolIdRegistry,
) -> Result<Option<Vec<u8>>, EntitySyncEncodeError> {
    let Some(protocol_id) = registry.protocol_id(effect.effect.as_str()) else {
        return if registry.is_empty() {
            Ok(None)
        } else {
            Err(EntitySyncEncodeError::MissingProtocolId {
                id: effect.effect.as_str().to_owned(),
            })
        };
    };
    let duration = i32::try_from(effect.duration_ticks).map_err(|_| {
        EntitySyncEncodeError::DurationOutOfRange {
            duration: effect.duration_ticks,
        }
    })?;
    let mut output = Vec::new();
    write_entity_id(&mut output, entity_id)?;
    write_varint(&mut output, protocol_id);
    write_varint(&mut output, i32::from(effect.amplifier));
    write_varint(&mut output, duration);
    let flags = u8::from(effect.ambient)
        | (u8::from(effect.visible) << 1)
        | (u8::from(effect.show_icon) << 2);
    output.push(flags);
    Ok(Some(output))
}

pub fn encode_remove_mob_effect(
    entity_id: EntityId,
    effect: &str,
    registry: &ProtocolIdRegistry,
) -> Result<Option<Vec<u8>>, EntitySyncEncodeError> {
    let Some(protocol_id) = registry.protocol_id(effect) else {
        return if registry.is_empty() {
            Ok(None)
        } else {
            Err(EntitySyncEncodeError::MissingProtocolId {
                id: effect.to_owned(),
            })
        };
    };
    let mut output = Vec::new();
    write_entity_id(&mut output, entity_id)?;
    write_varint(&mut output, protocol_id);
    Ok(Some(output))
}

pub fn encode_damage_event(
    entity_id: EntityId,
    source: DamageSource,
    damage_types: &ProtocolIdRegistry,
) -> Result<Option<Vec<u8>>, EntitySyncEncodeError> {
    let damage_type = damage_type_id(source.kind);
    let Some(protocol_id) = damage_types.protocol_id(damage_type) else {
        return if damage_types.is_empty() {
            Ok(None)
        } else {
            Err(EntitySyncEncodeError::MissingProtocolId {
                id: damage_type.to_owned(),
            })
        };
    };
    let mut output = Vec::new();
    write_entity_id(&mut output, entity_id)?;
    write_varint(&mut output, protocol_id);
    write_optional_entity_id(&mut output, source.attacker)?;
    write_optional_entity_id(&mut output, source.direct_entity)?;
    // The gameplay damage source currently has no fixed world position.
    output.push(0);
    Ok(Some(output))
}

pub fn encode_set_entity_motion(
    entity_id: EntityId,
    velocity: Velocity,
) -> Result<Vec<u8>, EntitySyncEncodeError> {
    let mut output = Vec::new();
    write_entity_id(&mut output, entity_id)?;
    for component in velocity.0 {
        if !component.is_finite() {
            return Err(EntitySyncEncodeError::NonFiniteVelocity);
        }
        let value = (component.clamp(-MAX_PACKET_VELOCITY, MAX_PACKET_VELOCITY)
            * VELOCITY_SCALE) as i16;
        output.extend_from_slice(&value.to_be_bytes());
    }
    Ok(output)
}

const fn damage_type_id(kind: DamageKind) -> &'static str {
    match kind {
        DamageKind::Generic => "minecraft:generic",
        DamageKind::PlayerAttack => "minecraft:player_attack",
        DamageKind::MobAttack => "minecraft:mob_attack",
        DamageKind::Projectile => "minecraft:arrow",
        DamageKind::Fall => "minecraft:fall",
        DamageKind::Fire => "minecraft:on_fire",
        DamageKind::Drowning => "minecraft:drown",
        DamageKind::Explosion => "minecraft:explosion",
        DamageKind::Void => "minecraft:out_of_world",
    }
}

fn write_optional_entity_id(
    output: &mut Vec<u8>,
    entity_id: Option<EntityId>,
) -> Result<(), EntitySyncEncodeError> {
    let value = match entity_id {
        Some(entity_id) => i32::try_from(entity_id.get())
            .map_err(|_| EntitySyncEncodeError::EntityIdOutOfRange {
                entity_id: entity_id.get(),
            })?
            .checked_add(1)
            .ok_or(EntitySyncEncodeError::EntityIdOutOfRange {
                entity_id: entity_id.get(),
            })?,
        None => 0,
    };
    write_varint(output, value);
    Ok(())
}

fn write_entity_id(
    output: &mut Vec<u8>,
    entity_id: EntityId,
) -> Result<(), EntitySyncEncodeError> {
    let value = i32::try_from(entity_id.get()).map_err(|_| {
        EntitySyncEncodeError::EntityIdOutOfRange {
            entity_id: entity_id.get(),
        }
    })?;
    write_varint(output, value);
    Ok(())
}

fn write_identifier(
    output: &mut Vec<u8>,
    identifier: &str,
) -> Result<(), EntitySyncEncodeError> {
    validate_identifier(identifier)?;
    write_len(output, identifier.len())?;
    output.extend_from_slice(identifier.as_bytes());
    Ok(())
}

fn write_len(output: &mut Vec<u8>, value: usize) -> Result<(), EntitySyncEncodeError> {
    let value = i32::try_from(value)
        .map_err(|_| EntitySyncEncodeError::LengthOutOfRange { length: value })?;
    write_varint(output, value);
    Ok(())
}

fn write_varint(output: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        if value & !0x7f == 0 {
            output.push(value as u8);
            break;
        }
        output.push(((value & 0x7f) | 0x80) as u8);
        value >>= 7;
    }
}

fn validate_identifier(identifier: &str) -> Result<(), EntitySyncEncodeError> {
    let Some((namespace, path)) = identifier.split_once(':') else {
        return Err(EntitySyncEncodeError::InvalidIdentifier {
            id: identifier.to_owned(),
        });
    };
    let valid = !namespace.is_empty()
        && !path.is_empty()
        && identifier.len() <= MAX_IDENTIFIER_BYTES
        && namespace.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'.')
        })
        && path.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'.' | b'/')
        })
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..");
    if valid {
        Ok(())
    } else {
        Err(EntitySyncEncodeError::InvalidIdentifier {
            id: identifier.to_owned(),
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EntitySyncEncodeError {
    #[error("registry identifier is invalid: {id}")]
    InvalidIdentifier { id: String },
    #[error("registry entry is duplicated: {id}")]
    DuplicateRegistryEntry { id: String },
    #[error("registry entry {id} has negative protocol ID {protocol_id}")]
    NegativeProtocolId { id: String, protocol_id: i32 },
    #[error("protocol ID {protocol_id} is shared by {first} and {second}")]
    DuplicateProtocolId {
        protocol_id: i32,
        first: String,
        second: String,
    },
    #[error("generated protocol registry is missing {id}")]
    MissingProtocolId { id: String },
    #[error("entity ID {entity_id} exceeds the protocol i32 range")]
    EntityIdOutOfRange { entity_id: u32 },
    #[error("attribute count {count} is outside 1..={MAX_SYNC_ATTRIBUTES}")]
    AttributeCountOutOfRange { count: usize },
    #[error("attribute modifier count {count} exceeds {MAX_SYNC_MODIFIERS}")]
    ModifierCountOutOfRange { count: usize },
    #[error("status-effect duration {duration} exceeds the protocol i32 range")]
    DurationOutOfRange { duration: u32 },
    #[error("collection or identifier length {length} exceeds the protocol range")]
    LengthOutOfRange { length: usize },
    #[error("entity velocity is not finite")]
    NonFiniteVelocity,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_game::{AttributeModifier, StatusEffectId};

    fn entity_id() -> EntityId {
        EntityId::new(7).unwrap()
    }

    #[test]
    fn encodes_generated_attribute_ids_and_modifiers() {
        let registry = ProtocolIdRegistry::new([
            ("minecraft:armor", 0),
            ("minecraft:movement_speed", 22),
        ])
        .unwrap();
        let mut attributes = AttributeSet::new();
        let mut speed = AttributeInstance::new(0.1, 0.0, 1_024.0).unwrap();
        speed
            .insert_modifier(
                AttributeModifier::new(
                    "test:sprint",
                    0.3,
                    AttributeOperation::AddMultipliedTotal,
                )
                .unwrap(),
            )
            .unwrap();
        attributes
            .insert(ferrum_game::AttributeId::new("minecraft:movement_speed").unwrap(), speed)
            .unwrap();
        let payload = encode_update_attributes(entity_id(), &attributes, &registry)
            .unwrap()
            .unwrap();
        assert_eq!(&payload[..3], &[7, 1, 22]);
        assert_eq!(&payload[3..11], &0.1_f64.to_be_bytes());
        assert_eq!(payload[11], 1);
        assert_eq!(payload[12], 11);
        assert_eq!(&payload[13..24], b"test:sprint");
        assert_eq!(&payload[24..32], &0.3_f64.to_be_bytes());
        assert_eq!(payload[32], 2);
    }

    #[test]
    fn encodes_effect_flags_and_removal() {
        let registry = ProtocolIdRegistry::new([("minecraft:haste", 2)]).unwrap();
        let mut effect = StatusEffectInstance::new(
            StatusEffectId::new("minecraft:haste").unwrap(),
            1,
            200,
        )
        .unwrap();
        effect.ambient = true;
        effect.show_icon = false;
        assert_eq!(
            encode_update_mob_effect(entity_id(), &effect, &registry)
                .unwrap()
                .unwrap(),
            vec![7, 2, 1, 0xc8, 0x01, 3]
        );
        assert_eq!(
            encode_remove_mob_effect(entity_id(), "minecraft:haste", &registry)
                .unwrap()
                .unwrap(),
            vec![7, 2]
        );
    }

    #[test]
    fn damage_event_uses_optional_entity_ids_and_registry_order() {
        let registry = ProtocolIdRegistry::new([("minecraft:player_attack", 34)]).unwrap();
        let attacker = EntityId::new(9).unwrap();
        let payload = encode_damage_event(
            entity_id(),
            DamageSource {
                kind: DamageKind::PlayerAttack,
                attacker: Some(attacker),
                direct_entity: Some(attacker),
                bypasses_armor: false,
                bypasses_invulnerability: false,
            },
            &registry,
        )
        .unwrap()
        .unwrap();
        assert_eq!(payload, vec![7, 34, 10, 10, 0]);
    }

    #[test]
    fn entity_motion_uses_limited_precision_shorts() {
        let velocity = Velocity::new([0.4, -0.25, 4.5]).unwrap();
        assert_eq!(
            encode_set_entity_motion(entity_id(), velocity).unwrap(),
            vec![7, 0x0c, 0x80, 0xf8, 0x30, 0x79, 0xe0]
        );
    }

    #[test]
    fn unknown_generated_ids_are_never_guessed() {
        let effect = StatusEffectInstance::new(
            StatusEffectId::new("minecraft:speed").unwrap(),
            0,
            20,
        )
        .unwrap();
        assert!(
            encode_update_mob_effect(
                entity_id(),
                &effect,
                &ProtocolIdRegistry::default()
            )
            .unwrap()
            .is_none()
        );
    }
}
