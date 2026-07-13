use std::collections::{BTreeMap, BTreeSet};

use ferrum_game::{EntityId, GameMode, PlayerUuid, Transform, Velocity};
use thiserror::Error;

const POSITION_SCALE: f64 = 4096.0;
const VELOCITY_SCALE: f64 = 8000.0;
const MAX_PACKET_VELOCITY: f64 = 3.9;
const PLAYER_INFO_ACTION_BYTES: usize = 1;
const PLAYER_INFO_ALL_ACTIONS: u8 = 0xff;
const MAX_USERNAME_BYTES: usize = 16;
const ENTITY_DATA_TERMINATOR: u8 = 0xff;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntityProtocolRegistry {
    ids: BTreeMap<String, i32>,
    names: BTreeMap<i32, String>,
}

impl EntityProtocolRegistry {
    pub fn new<I, S>(entries: I) -> Result<Self, EntityEncodeError>
    where
        I: IntoIterator<Item = (S, i32)>,
        S: Into<String>,
    {
        let mut ids = BTreeMap::new();
        let mut names = BTreeMap::new();
        for (entity_type, protocol_id) in entries {
            let entity_type = entity_type.into();
            validate_registry_entry(&entity_type, protocol_id)?;
            if ids.insert(entity_type.clone(), protocol_id).is_some() {
                return Err(EntityEncodeError::DuplicateEntityType { entity_type });
            }
            if names.insert(protocol_id, entity_type).is_some() {
                return Err(EntityEncodeError::DuplicateEntityProtocolId { protocol_id });
            }
        }
        Ok(Self { ids, names })
    }

    #[must_use]
    pub fn protocol_id(&self, entity_type: &str) -> Option<i32> {
        self.ids.get(entity_type).copied()
    }

    #[must_use]
    pub fn entity_type(&self, protocol_id: i32) -> Option<&str> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfoEntry {
    pub uuid: PlayerUuid,
    pub name: String,
    pub game_mode: GameMode,
    pub listed: bool,
    pub latency: i32,
    pub list_order: i32,
    pub show_hat: bool,
}

impl PlayerInfoEntry {
    #[must_use]
    pub fn new(uuid: PlayerUuid, name: impl Into<String>, game_mode: GameMode) -> Self {
        Self {
            uuid,
            name: name.into(),
            game_mode,
            listed: true,
            latency: 0,
            list_order: 0,
            show_hat: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityMovementKind {
    Position,
    PositionRotation,
    Rotation,
    Teleport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedEntityMovement {
    pub kind: EntityMovementKind,
    pub payload: Vec<u8>,
}

pub fn encode_player_info_update(
    entries: &[PlayerInfoEntry],
) -> Result<Vec<u8>, EntityEncodeError> {
    let mut output = Vec::new();
    debug_assert_eq!(PLAYER_INFO_ACTION_BYTES, 1);
    output.push(PLAYER_INFO_ALL_ACTIONS);
    write_len(&mut output, entries.len())?;
    for entry in entries {
        validate_username(&entry.name)?;
        if entry.latency < 0 {
            return Err(EntityEncodeError::NegativeLatency {
                latency: entry.latency,
            });
        }
        if entry.list_order < 0 {
            return Err(EntityEncodeError::NegativeListOrder {
                list_order: entry.list_order,
            });
        }
        output.extend_from_slice(entry.uuid.as_bytes());

        // ADD_PLAYER: profile name, then the GameProfile property map. Offline
        // identities do not carry signed profile properties.
        write_string(&mut output, &entry.name)?;
        write_varint(&mut output, 0);

        // INITIALIZE_CHAT: no remote chat session for offline-mode players.
        output.push(0);

        // UPDATE_GAME_MODE, UPDATE_LISTED, UPDATE_LATENCY.
        write_varint(&mut output, game_mode_id(entry.game_mode));
        output.push(u8::from(entry.listed));
        write_varint(&mut output, entry.latency);

        // UPDATE_DISPLAY_NAME: no custom component.
        output.push(0);

        // UPDATE_LIST_ORDER and UPDATE_HAT.
        write_varint(&mut output, entry.list_order);
        output.push(u8::from(entry.show_hat));
    }
    Ok(output)
}

pub fn encode_player_info_remove(uuids: &[PlayerUuid]) -> Result<Vec<u8>, EntityEncodeError> {
    let mut output = Vec::new();
    write_len(&mut output, uuids.len())?;
    for uuid in uuids {
        output.extend_from_slice(uuid.as_bytes());
    }
    Ok(output)
}

pub fn encode_add_entity(
    entity_id: EntityId,
    uuid: PlayerUuid,
    entity_type: &str,
    transform: Transform,
    velocity: Velocity,
    registry: &EntityProtocolRegistry,
) -> Result<Option<Vec<u8>>, EntityEncodeError> {
    let Some(protocol_id) = registry.protocol_id(entity_type) else {
        return Ok(None);
    };
    let mut output = Vec::new();
    write_entity_id(&mut output, entity_id)?;
    output.extend_from_slice(uuid.as_bytes());
    write_varint(&mut output, protocol_id);
    for coordinate in transform.position {
        if !coordinate.is_finite() {
            return Err(EntityEncodeError::NonFiniteCoordinate);
        }
        output.extend_from_slice(&coordinate.to_be_bytes());
    }
    for component in velocity.0 {
        output.extend_from_slice(&encode_velocity_component(component)?.to_be_bytes());
    }
    output.push(pack_degrees(transform.pitch));
    output.push(pack_degrees(transform.yaw));
    output.push(pack_degrees(transform.yaw));
    write_varint(&mut output, 0);
    Ok(Some(output))
}

pub fn encode_remove_entities(entity_ids: &[EntityId]) -> Result<Vec<u8>, EntityEncodeError> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    write_len(&mut output, entity_ids.len())?;
    for entity_id in entity_ids {
        if !seen.insert(*entity_id) {
            return Err(EntityEncodeError::DuplicateEntityId {
                entity_id: entity_id.get(),
            });
        }
        write_entity_id(&mut output, *entity_id)?;
    }
    Ok(output)
}

pub fn encode_rotate_head(
    entity_id: EntityId,
    yaw: f32,
) -> Result<Vec<u8>, EntityEncodeError> {
    validate_rotation(yaw)?;
    let mut output = Vec::new();
    write_entity_id(&mut output, entity_id)?;
    output.push(pack_degrees(yaw));
    Ok(output)
}

pub fn encode_empty_entity_data(entity_id: EntityId) -> Result<Vec<u8>, EntityEncodeError> {
    let mut output = Vec::new();
    write_entity_id(&mut output, entity_id)?;
    output.push(ENTITY_DATA_TERMINATOR);
    Ok(output)
}

pub fn encode_entity_movement(
    entity_id: EntityId,
    from: Transform,
    to: Transform,
) -> Result<Option<EncodedEntityMovement>, EntityEncodeError> {
    validate_transform(from)?;
    validate_transform(to)?;
    let rotation_changed = pack_degrees(from.yaw) != pack_degrees(to.yaw)
        || pack_degrees(from.pitch) != pack_degrees(to.pitch);
    let position_changed = from.position != to.position;

    if !position_changed {
        if !rotation_changed {
            return Ok(None);
        }
        let mut output = Vec::new();
        write_entity_id(&mut output, entity_id)?;
        output.push(pack_degrees(to.yaw));
        output.push(pack_degrees(to.pitch));
        output.push(u8::from(to.on_ground));
        return Ok(Some(EncodedEntityMovement {
            kind: EntityMovementKind::Rotation,
            payload: output,
        }));
    }

    let deltas = relative_deltas(from.position, to.position);
    if let Some([delta_x, delta_y, delta_z]) = deltas {
        let mut output = Vec::new();
        write_entity_id(&mut output, entity_id)?;
        output.extend_from_slice(&delta_x.to_be_bytes());
        output.extend_from_slice(&delta_y.to_be_bytes());
        output.extend_from_slice(&delta_z.to_be_bytes());
        let kind = if rotation_changed {
            output.push(pack_degrees(to.yaw));
            output.push(pack_degrees(to.pitch));
            EntityMovementKind::PositionRotation
        } else {
            EntityMovementKind::Position
        };
        output.push(u8::from(to.on_ground));
        return Ok(Some(EncodedEntityMovement {
            kind,
            payload: output,
        }));
    }

    Ok(Some(EncodedEntityMovement {
        kind: EntityMovementKind::Teleport,
        payload: encode_teleport_entity(entity_id, to, Velocity::default())?,
    }))
}

pub fn encode_teleport_entity(
    entity_id: EntityId,
    transform: Transform,
    velocity: Velocity,
) -> Result<Vec<u8>, EntityEncodeError> {
    validate_transform(transform)?;
    let mut output = Vec::new();
    write_entity_id(&mut output, entity_id)?;

    // PositionMoveRotation.STREAM_CODEC: absolute position Vec3, delta
    // movement Vec3, yRot float, xRot float.
    for coordinate in transform.position {
        output.extend_from_slice(&coordinate.to_be_bytes());
    }
    for component in velocity.0 {
        if !component.is_finite() {
            return Err(EntityEncodeError::NonFiniteVelocity);
        }
        output.extend_from_slice(&component.to_be_bytes());
    }
    output.extend_from_slice(&transform.yaw.to_be_bytes());
    output.extend_from_slice(&transform.pitch.to_be_bytes());

    // Relative.SET_STREAM_CODEC is an INT mask. This packet is absolute.
    output.extend_from_slice(&0_i32.to_be_bytes());
    output.push(u8::from(transform.on_ground));
    Ok(output)
}

fn relative_deltas(from: [f64; 3], to: [f64; 3]) -> Option<[i16; 3]> {
    let mut result = [0_i16; 3];
    for axis in 0..3 {
        let from_packet = packet_coordinate(from[axis]);
        let to_packet = packet_coordinate(to[axis]);
        let delta = to_packet.checked_sub(from_packet)?;
        result[axis] = i16::try_from(delta).ok()?;
    }
    Some(result)
}

fn packet_coordinate(value: f64) -> i64 {
    (value * POSITION_SCALE).floor() as i64
}

fn encode_velocity_component(value: f64) -> Result<i16, EntityEncodeError> {
    if !value.is_finite() {
        return Err(EntityEncodeError::NonFiniteVelocity);
    }
    let scaled = value.clamp(-MAX_PACKET_VELOCITY, MAX_PACKET_VELOCITY) * VELOCITY_SCALE;
    Ok(scaled as i16)
}

fn validate_registry_entry(name: &str, protocol_id: i32) -> Result<(), EntityEncodeError> {
    if !is_resource_location(name) {
        return Err(EntityEncodeError::InvalidEntityType {
            entity_type: name.to_owned(),
        });
    }
    if protocol_id < 0 {
        return Err(EntityEncodeError::NegativeEntityProtocolId { protocol_id });
    }
    Ok(())
}

fn validate_username(name: &str) -> Result<(), EntityEncodeError> {
    if !(3..=MAX_USERNAME_BYTES).contains(&name.len())
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(EntityEncodeError::InvalidUsername {
            name: name.to_owned(),
        });
    }
    Ok(())
}

fn validate_transform(transform: Transform) -> Result<(), EntityEncodeError> {
    if transform.position.into_iter().any(|value| !value.is_finite()) {
        return Err(EntityEncodeError::NonFiniteCoordinate);
    }
    validate_rotation(transform.yaw)?;
    validate_rotation(transform.pitch)
}

fn validate_rotation(rotation: f32) -> Result<(), EntityEncodeError> {
    if rotation.is_finite() {
        Ok(())
    } else {
        Err(EntityEncodeError::NonFiniteRotation)
    }
}

fn game_mode_id(game_mode: GameMode) -> i32 {
    match game_mode {
        GameMode::Survival => 0,
        GameMode::Creative => 1,
        GameMode::Adventure => 2,
        GameMode::Spectator => 3,
    }
}

fn pack_degrees(degrees: f32) -> u8 {
    ((degrees * 256.0 / 360.0).floor() as i32) as u8
}

fn write_entity_id(output: &mut Vec<u8>, entity_id: EntityId) -> Result<(), EntityEncodeError> {
    let value = i32::try_from(entity_id.get()).map_err(|_| EntityEncodeError::EntityIdOutOfRange {
        entity_id: entity_id.get(),
    })?;
    write_varint(output, value);
    Ok(())
}

fn write_string(output: &mut Vec<u8>, value: &str) -> Result<(), EntityEncodeError> {
    write_len(output, value.len())?;
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_len(output: &mut Vec<u8>, value: usize) -> Result<(), EntityEncodeError> {
    let value = i32::try_from(value).map_err(|_| EntityEncodeError::LengthOutOfRange { value })?;
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

fn is_resource_location(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && namespace
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_- .".contains(&byte))
        && path.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'.' | b'/')
        })
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EntityEncodeError {
    #[error("entity type ID is invalid: {entity_type}")]
    InvalidEntityType { entity_type: String },
    #[error("entity type is duplicated: {entity_type}")]
    DuplicateEntityType { entity_type: String },
    #[error("entity protocol ID {protocol_id} is duplicated")]
    DuplicateEntityProtocolId { protocol_id: i32 },
    #[error("entity protocol ID {protocol_id} cannot be negative")]
    NegativeEntityProtocolId { protocol_id: i32 },
    #[error("entity ID {entity_id} exceeds the protocol i32 range")]
    EntityIdOutOfRange { entity_id: u32 },
    #[error("entity ID {entity_id} is duplicated")]
    DuplicateEntityId { entity_id: u32 },
    #[error("entity coordinate is not finite")]
    NonFiniteCoordinate,
    #[error("entity rotation is not finite")]
    NonFiniteRotation,
    #[error("entity velocity is not finite")]
    NonFiniteVelocity,
    #[error("player username is invalid: {name}")]
    InvalidUsername { name: String },
    #[error("player latency {latency} cannot be negative")]
    NegativeLatency { latency: i32 },
    #[error("player list order {list_order} cannot be negative")]
    NegativeListOrder { list_order: i32 },
    #[error("collection or string length {value} exceeds the protocol range")]
    LengthOutOfRange { value: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid() -> PlayerUuid {
        PlayerUuid::from_bytes([
            0x56, 0x27, 0xdd, 0x98, 0xe6, 0xbe, 0x3c, 0x21, 0xb8, 0xa8, 0xe9, 0x23, 0x44,
            0x18, 0x36, 0x41,
        ])
    }

    fn entity_id() -> EntityId {
        EntityId::new(7).unwrap()
    }

    #[test]
    fn player_info_uses_fixed_eight_action_bitset() {
        let payload = encode_player_info_update(&[PlayerInfoEntry::new(
            uuid(),
            "Steve",
            GameMode::Survival,
        )])
        .unwrap();
        assert_eq!(payload[0], 0xff);
        assert_eq!(payload[1], 1);
        assert_eq!(&payload[2..18], uuid().as_bytes());
        assert_eq!(&payload[18..24], &[5, b'S', b't', b'e', b'v', b'e']);
        assert_eq!(&payload[24..], &[0, 0, 0, 1, 0, 0, 0, 1]);
    }

    #[test]
    fn add_entity_uses_generated_entity_type_id() {
        let registry =
            EntityProtocolRegistry::new([("minecraft:player", 155)]).unwrap();
        let transform = Transform::new([1.0, 65.0, -2.0], 90.0, 45.0, true).unwrap();
        let payload = encode_add_entity(
            entity_id(),
            uuid(),
            "minecraft:player",
            transform,
            Velocity::default(),
            &registry,
        )
        .unwrap()
        .unwrap();
        assert_eq!(payload[0], 7);
        assert_eq!(&payload[1..17], uuid().as_bytes());
        assert_eq!(&payload[17..19], &[0x9b, 0x01]);
        assert_eq!(payload[payload.len() - 4..], [32, 64, 64, 0]);
    }

    #[test]
    fn relative_position_rotation_uses_packet_coordinate_deltas() {
        let from = Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap();
        let to = Transform::new([0.75, 65.5, 0.25], 90.0, -45.0, true).unwrap();
        let movement = encode_entity_movement(entity_id(), from, to)
            .unwrap()
            .unwrap();
        assert_eq!(movement.kind, EntityMovementKind::PositionRotation);
        assert_eq!(
            movement.payload,
            vec![7, 0x04, 0x00, 0x08, 0x00, 0xfc, 0x00, 64, 224, 1]
        );
    }

    #[test]
    fn large_position_delta_falls_back_to_absolute_teleport() {
        let from = Transform::new([0.0, 65.0, 0.0], 0.0, 0.0, true).unwrap();
        let to = Transform::new([20.0, 65.0, 0.0], 0.0, 0.0, true).unwrap();
        let movement = encode_entity_movement(entity_id(), from, to)
            .unwrap()
            .unwrap();
        assert_eq!(movement.kind, EntityMovementKind::Teleport);
        assert_eq!(movement.payload.len(), 66);
    }

    #[test]
    fn empty_metadata_is_terminated_with_ff() {
        assert_eq!(encode_empty_entity_data(entity_id()).unwrap(), vec![7, 0xff]);
    }

    #[test]
    fn unknown_entity_type_is_skipped_without_guessing() {
        assert!(
            encode_add_entity(
                entity_id(),
                uuid(),
                "minecraft:player",
                Transform::default(),
                Velocity::default(),
                &EntityProtocolRegistry::default(),
            )
            .unwrap()
            .is_none()
        );
    }
}
