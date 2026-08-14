from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected one marker in {path}, found {count}: {old[:180]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")

# rom-protocol: make the 26.1.2 dedicated attack packet a first-class semantic kind.
protocol = ROOT / "crates/rom-protocol/src/lib.rs"
replace_once(protocol, "    ClientCommand,\n    MovePlayerPosition,", "    ClientCommand,\n    Attack,\n    MovePlayerPosition,")
replace_once(protocol, "        Self::ClientCommand,\n        Self::MovePlayerPosition,", "        Self::ClientCommand,\n        Self::Attack,\n        Self::MovePlayerPosition,")
replace_once(protocol, "        Self::ClientTickEnd,\n        Self::MovePlayerPosition,", "        Self::ClientTickEnd,\n        Self::Attack,\n        Self::MovePlayerPosition,")
replace_once(protocol, "            | Self::ClientCommand\n            | Self::MovePlayerPosition", "            | Self::ClientCommand\n            | Self::Attack\n            | Self::MovePlayerPosition")
replace_once(protocol, "            | Self::ClientCommand\n            | Self::MovePlayerPosition", "            | Self::ClientCommand\n            | Self::Attack\n            | Self::MovePlayerPosition")

catalog = ROOT / "crates/rom-protocol/src/packet_catalog.rs"
replace_once(
    catalog,
    "        (ProtocolPhase::Play, PacketDirection::Serverbound, \"client_command\") => {\n            Some(PacketKind::ClientCommand)\n        }\n",
    "        (ProtocolPhase::Play, PacketDirection::Serverbound, \"client_command\") => {\n            Some(PacketKind::ClientCommand)\n        }\n        (ProtocolPhase::Play, PacketDirection::Serverbound, \"attack\") => {\n            Some(PacketKind::Attack)\n        }\n",
)
replace_once(catalog, "        PacketKind::ClientCommand => \"minecraft:client_command\",\n", "        PacketKind::ClientCommand => \"minecraft:client_command\",\n        PacketKind::Attack => \"minecraft:attack\",\n")
replace_once(
    catalog,
    "    #[test]\n    fn recognizes_set_player_inventory_as_optional_typed_packet() {",
    "    #[test]\n    fn recognizes_attack_as_serverbound_play_packet() {\n        assert_eq!(\n            known_packet_kind(\n                ProtocolPhase::Play,\n                PacketDirection::Serverbound,\n                \"minecraft:attack\",\n            ),\n            Some(PacketKind::Attack)\n        );\n        assert_eq!(canonical_packet_name(PacketKind::Attack), \"minecraft:attack\");\n    }\n\n    #[test]\n    fn recognizes_set_player_inventory_as_optional_typed_packet() {",
)

version = ROOT / "crates/rom-version-26-1-2/src/lib.rs"
replace_once(version, "        (PacketKind::AcceptTeleportation, 0x00),\n", "        (PacketKind::AcceptTeleportation, 0x00),\n        (PacketKind::Attack, 0x01),\n")
replace_once(
    version,
    "        assert_eq!(\n            packets.require(PacketKind::AcceptTeleportation).unwrap(),\n            0x00\n        );\n",
    "        assert_eq!(\n            packets.require(PacketKind::AcceptTeleportation).unwrap(),\n            0x00\n        );\n        assert_eq!(packets.require(PacketKind::Attack).unwrap(), 0x01);\n",
)

# Semantic Play input decoder. ServerboundAttackPacket is a single VarInt entity id.
play_input = ROOT / "crates/rom-server/src/play_input.rs"
replace_once(
    play_input,
    "        PacketKind::KeepAliveResponse => {",
    "        PacketKind::Attack => PlayInput::AttackEntity(decode_positive_varint(\"attack entity id\", payload)?),\n        PacketKind::KeepAliveResponse => {",
)
replace_once(
    play_input,
    "fn require_length(name: &str, payload: &[u8], expected: usize) -> Result<()> {",
    '''fn decode_positive_varint(name: &str, payload: &[u8]) -> Result<u32> {\n    let mut value = 0_u32;\n    let mut cursor = 0_usize;\n    for shift in (0..35).step_by(7) {\n        let Some(&byte) = payload.get(cursor) else {\n            bail!("{name} contains a truncated VarInt");\n        };\n        cursor += 1;\n        value |= u32::from(byte & 0x7f) << shift;\n        if byte & 0x80 == 0 {\n            if cursor != payload.len() {\n                bail!("{name} payload contains trailing bytes");\n            }\n            let signed = value as i32;\n            if signed <= 0 {\n                bail!("{name} must be a positive entity id, got {signed}");\n            }\n            return Ok(signed as u32);\n        }\n    }\n    bail!("{name} VarInt is too long")\n}\n\nfn require_length(name: &str, payload: &[u8], expected: usize) -> Result<()> {''',
)
replace_once(
    play_input,
    "    #[test]\n    fn validates_chunk_batch_acknowledgements() {",
    '''    #[test]\n    fn decodes_dedicated_attack_entity_id() {\n        assert_eq!(\n            decode_play_input(PacketKind::Attack, &[0xac, 0x02]).unwrap(),\n            Some(PlayInput::AttackEntity(300))\n        );\n        assert!(decode_play_input(PacketKind::Attack, &[0]).is_err());\n        assert!(decode_play_input(PacketKind::Attack, &[1, 0]).is_err());\n    }\n\n    #[test]\n    fn validates_chunk_batch_acknowledgements() {''',
)

# Worker runtime understands and records the semantic attack input.
auth = ROOT / "crates/rom-server/src/authoritative_runtime.rs"
replace_once(auth, "    KeepAliveResponse(i64),\n    Movement(PlayerMovement),", "    KeepAliveResponse(i64),\n    AttackEntity(u32),\n    Movement(PlayerMovement),")
replace_once(auth, "    pub last_movement: Option<PlayerMovement>,\n", "    pub last_movement: Option<PlayerMovement>,\n    pub last_attack_entity: Option<u32>,\n")
replace_once(
    auth,
    "                PlayInput::Movement(movement) => {",
    "                PlayInput::AttackEntity(entity_id) => {\n                    self.connections\n                        .entry(input.connection)\n                        .or_default()\n                        .last_attack_entity = Some(entity_id);\n                }\n                PlayInput::Movement(movement) => {",
)

# Isolate combat policy from the already-large play loop.
combat = ROOT / "crates/rom-server/src/combat_runtime.rs"
combat.write_text(r'''use rom_game::{GameEvent, GameMode, HOTBAR_START, PlayerUuid};

use crate::game_runtime::{GameRuntimeError, SharedGameRuntime};

pub const PLAYER_ATTACK_REACH: f64 = 3.0;
pub const PLAYER_ATTACK_KNOCKBACK: f64 = 0.4;
pub const PLAYER_ATTACK_VERTICAL_KNOCKBACK: f64 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerAttackOutcome {
    pub target: PlayerUuid,
    pub damage: f32,
    pub killed: bool,
}

impl SharedGameRuntime {
    pub fn attack_player_entity(
        &self,
        attacker_uuid: PlayerUuid,
        protocol_entity_id: u32,
    ) -> Result<Option<PlayerAttackOutcome>, GameRuntimeError> {
        let snapshot = self.with_state(|state| {
            let attacker = state.player(attacker_uuid)?;
            if !attacker.connected
                || attacker.game_mode == GameMode::Spectator
                || attacker.vitals.is_dead()
            {
                return None;
            }
            let attacker_entity = attacker.entity_id?;
            if attacker_entity.get() == protocol_entity_id {
                return None;
            }
            let attacker_position = state.entities().get(attacker_entity)?.transform.position;
            let (target_uuid, target) = state.players().iter().find(|(_, player)| {
                player.connected
                    && player
                        .entity_id
                        .is_some_and(|entity_id| entity_id.get() == protocol_entity_id)
            })?;
            if target.abilities.invulnerable || target.vitals.is_dead() {
                return None;
            }
            let target_entity = target.entity_id?;
            let target_position = state.entities().get(target_entity)?.transform.position;
            let dx = target_position[0] - attacker_position[0];
            let dy = target_position[1] - attacker_position[1];
            let dz = target_position[2] - attacker_position[2];
            let distance_squared = dx * dx + dy * dy + dz * dz;
            if !distance_squared.is_finite()
                || distance_squared > PLAYER_ATTACK_REACH * PLAYER_ATTACK_REACH
            {
                return None;
            }
            let selected = HOTBAR_START + usize::from(attacker.inventory.selected_hotbar());
            let item = attacker.inventory.slot(selected).ok().flatten().map(|stack| stack.item());
            Some((*target_uuid, attacker_position, attack_damage_for_item(item)))
        })?;
        let Some((target_uuid, attacker_position, damage)) = snapshot else {
            return Ok(None);
        };

        let events = self.damage_player(target_uuid, damage)?;
        let killed = events
            .iter()
            .any(|event| matches!(event, GameEvent::PlayerKilled { uuid, .. } if *uuid == target_uuid));
        // Player entities remain authoritative through death until respawn, so velocity
        // can be replicated even for the lethal attack just like ordinary knockback.
        let _ = self.knockback_player(
            target_uuid,
            attacker_position,
            PLAYER_ATTACK_KNOCKBACK,
            PLAYER_ATTACK_VERTICAL_KNOCKBACK,
        )?;
        Ok(Some(PlayerAttackOutcome {
            target: target_uuid,
            damage,
            killed,
        }))
    }
}

#[must_use]
pub fn attack_damage_for_item(item: Option<&str>) -> f32 {
    match item {
        Some("minecraft:wooden_sword" | "minecraft:golden_sword") => 4.0,
        Some("minecraft:stone_sword") => 5.0,
        Some("minecraft:iron_sword") => 6.0,
        Some("minecraft:diamond_sword") => 7.0,
        Some("minecraft:netherite_sword") => 8.0,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rom_game::{ItemStack, Transform, Velocity};

    fn transform(x: f64) -> Transform {
        Transform::new([x, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn close_player_attack_applies_damage_and_knockback() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let attacker = PlayerUuid::new(100);
        let target = PlayerUuid::new(101);
        runtime.connect_player(attacker, "Attacker", transform(0.5)).unwrap();
        runtime.connect_player(target, "Target", transform(2.5)).unwrap();
        runtime
            .with_state_mut(|state| {
                let player = state.player_mut(attacker).unwrap();
                player
                    .inventory
                    .set_slot(
                        HOTBAR_START,
                        Some(ItemStack::with_max_count("minecraft:diamond_sword", 1, 1).unwrap()),
                    )
                    .map_err(|error| rom_game::GameStateError::Inventory(error).into())?;
                Ok(())
            })
            .unwrap();
        let target_entity = runtime
            .with_state(|state| state.player(target).unwrap().entity_id.unwrap())
            .unwrap();

        let outcome = runtime
            .attack_player_entity(attacker, target_entity.get())
            .unwrap()
            .unwrap();
        assert_eq!(outcome.damage, 7.0);
        assert!(!outcome.killed);
        let (health, velocity) = runtime
            .with_state(|state| {
                let player = state.player(target).unwrap();
                let velocity = state.entities().get(player.entity_id.unwrap()).unwrap().velocity;
                (player.vitals.health, velocity)
            })
            .unwrap();
        assert_eq!(health, 13.0);
        assert_ne!(velocity, Velocity::ZERO);
    }

    #[test]
    fn rejects_self_far_and_invulnerable_targets() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let attacker = PlayerUuid::new(110);
        let target = PlayerUuid::new(111);
        runtime.connect_player(attacker, "Attacker", transform(0.5)).unwrap();
        runtime.connect_player(target, "FarTarget", transform(10.5)).unwrap();
        let attacker_entity = runtime
            .with_state(|state| state.player(attacker).unwrap().entity_id.unwrap())
            .unwrap();
        let target_entity = runtime
            .with_state(|state| state.player(target).unwrap().entity_id.unwrap())
            .unwrap();
        assert!(runtime.attack_player_entity(attacker, attacker_entity.get()).unwrap().is_none());
        assert!(runtime.attack_player_entity(attacker, target_entity.get()).unwrap().is_none());

        runtime
            .with_state_mut(|state| {
                let attacker_entity = state.player(attacker).unwrap().entity_id.unwrap();
                state.entities_mut().get_mut(attacker_entity).unwrap().transform = transform(9.5);
                state.player_mut(target).unwrap().set_game_mode(GameMode::Creative);
                Ok(())
            })
            .unwrap();
        assert!(runtime.attack_player_entity(attacker, target_entity.get()).unwrap().is_none());
    }
}
''', encoding="utf-8")

server_lib = ROOT / "crates/rom-server/src/lib.rs"
replace_once(server_lib, "pub mod authoritative_runtime;\n", "pub mod authoritative_runtime;\npub mod combat_runtime;\n")

# The socket bridge routes the same decoded semantic input to gameplay.
play_runtime = ROOT / "crates/rom-server/src/play_runtime.rs"
replace_once(
    play_runtime,
    "    game_runtime::SharedGameRuntime,\n",
    "    combat_runtime::PlayerAttackOutcome,\n    game_runtime::SharedGameRuntime,\n",
)
replace_once(
    play_runtime,
    "    fn synchronize(self, player: &PlayerState) -> Result<()> {",
    "    fn attack_entity(self, entity_id: u32) -> Result<Option<PlayerAttackOutcome>> {\n        Ok(self.runtime.attack_player_entity(self.player_uuid, entity_id)?)\n    }\n\n    fn synchronize(self, player: &PlayerState) -> Result<()> {",
)
replace_once(
    play_runtime,
    "                    | PacketKind::ClientTickEnd\n                    | PacketKind::ChunkBatchReceived",
    "                    | PacketKind::ClientTickEnd\n                    | PacketKind::Attack\n                    | PacketKind::ChunkBatchReceived",
)
replace_once(
    play_runtime,
    "                        PlayInput::ClientTickEnd => {\n                            ticks_since_request = ticks_since_request.saturating_add(1);\n                        }\n                        PlayInput::ChunkBatchReceived(_) => {}",
    "                        PlayInput::ClientTickEnd => {\n                            ticks_since_request = ticks_since_request.saturating_add(1);\n                        }\n                        PlayInput::AttackEntity(entity_id) => {\n                            if let Some(gameplay) = gameplay {\n                                let _ = gameplay.attack_entity(entity_id)?;\n                            }\n                        }\n                        PlayInput::ChunkBatchReceived(_) => {}",
)

print("Integrated dedicated 26.1.2 attack input and authoritative player combat loop.")
