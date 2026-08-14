use rom_game::{EntityId, EntityUuid, Transform, Velocity};
use thiserror::Error;

use crate::EntityProtocolRegistry;

const VELOCITY_SCALE: f64 = 8000.0;
const MAX_PACKET_VELOCITY: f64 = 3.9;

#[derive(Debug, Error, PartialEq)]
pub enum GenericEntityEncodeError {
    #[error("entity id {0} exceeds the protocol VarInt range")]
    EntityIdOutOfRange(u32),
    #[error("entity position must be finite")]
    NonFinitePosition,
    #[error("entity rotation must be finite")]
    NonFiniteRotation,
    #[error("entity velocity must be finite")]
    NonFiniteVelocity,
}

/// Encodes the clientbound Add Entity body for any authoritative world entity.
///
/// The existing player-oriented helper predates generic entity storage and takes
/// a `PlayerUuid`. World entities use `EntityUuid`, so this codec intentionally
/// accepts the generic domain identifier while preserving the same wire shape.
pub fn encode_add_world_entity(
    entity_id: EntityId,
    uuid: EntityUuid,
    entity_type: &str,
    transform: Transform,
    velocity: Velocity,
    registry: &EntityProtocolRegistry,
) -> Result<Option<Vec<u8>>, GenericEntityEncodeError> {
    let Some(protocol_id) = registry.protocol_id(entity_type) else {
        return Ok(None);
    };

    if transform
        .position
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(GenericEntityEncodeError::NonFinitePosition);
    }
    if !transform.yaw.is_finite() || !transform.pitch.is_finite() {
        return Err(GenericEntityEncodeError::NonFiniteRotation);
    }

    let mut output = Vec::with_capacity(64);
    write_entity_id(&mut output, entity_id)?;
    output.extend_from_slice(uuid.as_bytes());
    write_varint(&mut output, protocol_id);

    for coordinate in transform.position {
        output.extend_from_slice(&coordinate.to_be_bytes());
    }
    for component in velocity.0 {
        output.extend_from_slice(&encode_velocity(component)?.to_be_bytes());
    }

    output.push(pack_degrees(transform.pitch));
    output.push(pack_degrees(transform.yaw));
    output.push(pack_degrees(transform.yaw));
    write_varint(&mut output, 0);
    Ok(Some(output))
}

fn write_entity_id(
    output: &mut Vec<u8>,
    entity_id: EntityId,
) -> Result<(), GenericEntityEncodeError> {
    let value = i32::try_from(entity_id.get())
        .map_err(|_| GenericEntityEncodeError::EntityIdOutOfRange(entity_id.get()))?;
    write_varint(output, value);
    Ok(())
}

fn encode_velocity(value: f64) -> Result<i16, GenericEntityEncodeError> {
    if !value.is_finite() {
        return Err(GenericEntityEncodeError::NonFiniteVelocity);
    }
    Ok((value.clamp(-MAX_PACKET_VELOCITY, MAX_PACKET_VELOCITY) * VELOCITY_SCALE) as i16)
}

fn pack_degrees(value: f32) -> u8 {
    ((value.rem_euclid(360.0) * 256.0 / 360.0).floor() as i32).rem_euclid(256) as u8
}

fn write_varint(output: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        if value & !0x7f == 0 {
            output.push(value as u8);
            return;
        }
        output.push(((value & 0x7f) | 0x80) as u8);
        value >>= 7;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_generic_uuid_and_registry_id() {
        let registry = EntityProtocolRegistry::new([("minecraft:item", 123)]).unwrap();
        let uuid = EntityUuid::new(0x00112233445566778899aabbccddeeff);
        let payload = encode_add_world_entity(
            EntityId::new(7).unwrap(),
            uuid,
            "minecraft:item",
            Transform::new([1.0, 2.0, 3.0], 90.0, 0.0, false).unwrap(),
            Velocity::new([0.1, 0.2, -0.1]).unwrap(),
            &registry,
        )
        .unwrap()
        .unwrap();
        assert_eq!(payload[0], 7);
        assert_eq!(&payload[1..17], uuid.as_bytes());
        assert_eq!(payload[17], 123);
    }

    #[test]
    fn missing_registry_entry_is_not_an_error() {
        let registry = EntityProtocolRegistry::default();
        assert!(
            encode_add_world_entity(
                EntityId::new(1).unwrap(),
                EntityUuid::new(1),
                "minecraft:item",
                Transform::default(),
                Velocity::default(),
                &registry,
            )
            .unwrap()
            .is_none()
        );
    }
}
