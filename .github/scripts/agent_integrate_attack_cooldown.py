from pathlib import Path

pg = Path('crates/rom-game/src/player_gameplay.rs')
s = pg.read_text()
s = s.replace(
'''    #[serde(default)]
    mining_session: Option<MiningSession>,
}''',
'''    #[serde(default)]
    mining_session: Option<MiningSession>,
    #[serde(default)]
    last_attack_tick: Option<u64>,
}''')
s = s.replace(
'''            fall_distance: 0.0,
            mining_session: None,
        }''',
'''            fall_distance: 0.0,
            mining_session: None,
            last_attack_tick: None,
        }''')
needle = '''    #[must_use]
    pub const fn mining_session(&self) -> Option<MiningSession> {
        self.mining_session
    }
'''
insert = needle + '''\n    #[must_use]\n    pub const fn last_attack_tick(&self) -> Option<u64> {\n        self.last_attack_tick\n    }\n\n    #[must_use]\n    pub fn attack_strength_scale(&self, current_tick: u64, attack_speed: f32) -> f32 {\n        if !attack_speed.is_finite() || attack_speed <= 0.0 {\n            return 1.0;\n        }\n        let Some(last_attack_tick) = self.last_attack_tick else {\n            return 1.0;\n        };\n        let elapsed = current_tick.saturating_sub(last_attack_tick) as f32;\n        let cooldown_ticks = 20.0 / attack_speed;\n        ((elapsed + 0.5) / cooldown_ticks).clamp(0.0, 1.0)\n    }\n\n    pub fn record_attack(&mut self, current_tick: u64) {\n        self.last_attack_tick = Some(current_tick);\n    }\n'''
if needle not in s:
    raise SystemExit('player gameplay insertion point missing')
s = s.replace(needle, insert, 1)
pg.write_text(s)

combat = Path('crates/rom-server/src/combat_runtime.rs')
s = combat.read_text()
s = s.replace(
'''pub struct PlayerAttackOutcome {
    pub target: PlayerUuid,
    pub damage: f32,
    pub killed: bool,
}''',
'''pub struct PlayerAttackOutcome {
    pub target: PlayerUuid,
    pub damage: f32,
    pub attack_strength: f32,
    pub critical: bool,
    pub killed: bool,
}''')
s = s.replace(
'''            let selected = HOTBAR_START + usize::from(attacker.inventory.selected_hotbar());
            let item = attacker
                .inventory
                .slot(selected)
                .ok()
                .flatten()
                .map(|stack| stack.item());
            Some((
                *target_uuid,
                attacker_position,
                attack_damage_for_item(item),
            ))''',
'''            let selected = HOTBAR_START + usize::from(attacker.inventory.selected_hotbar());
            let item = attacker
                .inventory
                .slot(selected)
                .ok()
                .flatten()
                .map(|stack| stack.item());
            let current_tick = state.time().game_time;
            let attack_speed = attack_speed_for_item(item);
            let attack_strength = attacker
                .gameplay
                .attack_strength_scale(current_tick, attack_speed);
            let base_damage = attack_damage_for_item(item);
            let charged_damage = base_damage * (0.2 + attack_strength * attack_strength * 0.8);
            let critical = attack_strength > 0.9
                && !state.entities().get(attacker_entity)?.transform.on_ground
                && attacker.gameplay.fall_distance() > 0.0;
            let damage = if critical {
                charged_damage * 1.5
            } else {
                charged_damage
            };
            Some((*target_uuid, attacker_position, current_tick, attack_strength, critical, damage))''')
s = s.replace(
'''        let Some((target_uuid, attacker_position, damage)) = snapshot else {
            return Ok(None);
        };

        let events = self.damage_player(target_uuid, damage)?;''',
'''        let Some((target_uuid, attacker_position, current_tick, attack_strength, critical, damage)) = snapshot else {
            return Ok(None);
        };

        self.with_state_mut(|state| {
            let attacker = state
                .player_mut(attacker_uuid)
                .ok_or(rom_game::GameStateError::UnknownPlayer { uuid: attacker_uuid })?;
            attacker.gameplay.record_attack(current_tick);
            Ok(())
        })?;

        let events = self.damage_player(target_uuid, damage)?;''')
s = s.replace(
'''        Ok(Some(PlayerAttackOutcome {
            target: target_uuid,
            damage,
            killed,
        }))''',
'''        Ok(Some(PlayerAttackOutcome {
            target: target_uuid,
            damage,
            attack_strength,
            critical,
            killed,
        }))''')
needle = '''#[must_use]
pub fn attack_damage_for_item(item: Option<&str>) -> f32 {
'''
insert = '''#[must_use]\npub fn attack_speed_for_item(item: Option<&str>) -> f32 {\n    match item {\n        Some(\n            "minecraft:wooden_sword"\n            | "minecraft:golden_sword"\n            | "minecraft:stone_sword"\n            | "minecraft:iron_sword"\n            | "minecraft:diamond_sword"\n            | "minecraft:netherite_sword",\n        ) => 1.6,\n        _ => 4.0,\n    }\n}\n\n''' + needle
if needle not in s:
    raise SystemExit('combat helper insertion point missing')
s = s.replace(needle, insert, 1)
s = s.replace(
'''        assert_eq!(outcome.damage, 7.0);
        assert!(!outcome.killed);''',
'''        assert_eq!(outcome.damage, 7.0);
        assert_eq!(outcome.attack_strength, 1.0);
        assert!(!outcome.critical);
        assert!(!outcome.killed);''')
# add cooldown unit test before final module brace
marker = '''    #[test]
    fn rejects_self_far_and_invulnerable_targets() {'''
if marker not in s:
    raise SystemExit('combat test marker missing')
extra = '''    #[test]\n    fn repeated_sword_attack_is_cooldown_scaled() {\n        let runtime = SharedGameRuntime::vanilla_overworld();\n        let attacker = PlayerUuid::new(120);\n        let target = PlayerUuid::new(121);\n        runtime.connect_player(attacker, "Attacker", transform(0.5)).unwrap();\n        runtime.connect_player(target, "Target", transform(2.5)).unwrap();\n        runtime.with_state_mut(|state| {\n            state.player_mut(attacker).unwrap().inventory.set_slot(\n                HOTBAR_START,\n                Some(ItemStack::with_max_count("minecraft:diamond_sword", 1, 1).unwrap()),\n            ).map_err(|error| GameRuntimeError::State(rom_game::GameStateError::Inventory(error)))?;\n            Ok(())\n        }).unwrap();\n        let target_entity = runtime.with_state(|state| state.player(target).unwrap().entity_id.unwrap()).unwrap();\n        let first = runtime.attack_player_entity(attacker, target_entity.get()).unwrap().unwrap();\n        let second = runtime.attack_player_entity(attacker, target_entity.get()).unwrap().unwrap();\n        assert_eq!(first.attack_strength, 1.0);\n        assert!(second.attack_strength < 0.1);\n        assert!(second.damage < 2.0);\n    }\n\n'''
s = s.replace(marker, extra + marker, 1)
combat.write_text(s)
print('Integrated authoritative attack cooldown and critical hits.')
