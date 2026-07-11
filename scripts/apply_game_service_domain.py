from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


state = Path("crates/ferrum-game/src/state.rs")
text = state.read_text(encoding="utf-8")
text = replace_once(
    text,
    "    SaveRequested,\n    Broadcast {",
    "    SaveRequested,\n    ShutdownRequested,\n    Broadcast {",
    "shutdown event",
)
text = replace_once(
    text,
    "    pub fn tick(&mut self) {",
    '''    pub fn detach_all_connections(&mut self) -> usize {
        let entity_ids = self
            .players
            .values_mut()
            .filter_map(|player| {
                if !player.connected {
                    return None;
                }
                let entity_id = player.entity_id;
                player.disconnect();
                entity_id
            })
            .collect::<Vec<_>>();
        for entity_id in &entity_ids {
            self.entities.despawn(*entity_id);
        }
        entity_ids.len()
    }

    pub fn tick(&mut self) {''',
    "detach all connections",
)
text = replace_once(
    text,
    "    fn daylight_cycle_rule_controls_day_time() {",
    '''    fn detaches_live_connections_for_restart() {
        let mut state = GameState::default();
        state
            .connect_player(PlayerUuid::new(20), "Steve", spawn())
            .unwrap();
        state
            .connect_player(PlayerUuid::new(21), "Alex", spawn())
            .unwrap();
        assert_eq!(state.detach_all_connections(), 2);
        assert_eq!(state.online_player_count(), 0);
        assert!(state.entities().is_empty());
        assert!(state.players().values().all(|player| player.entity_id.is_none()));
    }

    #[test]
    fn daylight_cycle_rule_controls_day_time() {''',
    "detach test",
)
state.write_text(text, encoding="utf-8")

command = Path("crates/ferrum-game/src/command.rs")
text = command.read_text(encoding="utf-8")
text = replace_once(
    text,
    "    SaveAll,\n}",
    "    SaveAll,\n    Stop,\n}",
    "stop command variant",
)
text = replace_once(
    text,
    "    pub save_requested: bool,\n}",
    "    pub save_requested: bool,\n    pub shutdown_requested: bool,\n}",
    "shutdown outcome flag",
)
text = replace_once(
    text,
    '        ["save-all"] | ["save-all", "flush"] => Ok(GameCommand::SaveAll),',
    '        ["save-all"] | ["save-all", "flush"] => Ok(GameCommand::SaveAll),\n        ["stop"] => Ok(GameCommand::Stop),',
    "stop parser",
)
text = text.replace(
    "                save_requested: false,\n",
    "                save_requested: false,\n                shutdown_requested: false,\n",
)
text = replace_once(
    text,
    '''            Ok(CommandOutcome {
                feedback: "Saved the game".to_owned(),
                events: vec![GameEvent::SaveRequested],
                save_requested: true,
            })
        }
    }
}''',
    '''            Ok(CommandOutcome {
                feedback: "Saved the game".to_owned(),
                events: vec![GameEvent::SaveRequested],
                save_requested: true,
                shutdown_requested: false,
            })
        }
        GameCommand::Stop => {
            require_permission(source, 4)?;
            Ok(CommandOutcome {
                feedback: "Stopping the server".to_owned(),
                events: vec![GameEvent::ShutdownRequested],
                save_requested: true,
                shutdown_requested: true,
            })
        }
    }
}''',
    "stop command execution",
)
text = replace_once(
    text,
    '''        assert!(outcome.save_requested);
        assert_eq!(outcome.events, [GameEvent::SaveRequested]);
    }
}''',
    '''        assert!(outcome.save_requested);
        assert!(!outcome.shutdown_requested);
        assert_eq!(outcome.events, [GameEvent::SaveRequested]);
    }

    #[test]
    fn stop_requests_save_and_shutdown() {
        let mut state = GameState::default();
        let outcome = execute_command(&mut state, &CommandSource::console(), "/stop").unwrap();
        assert!(outcome.save_requested);
        assert!(outcome.shutdown_requested);
        assert_eq!(outcome.events, [GameEvent::ShutdownRequested]);
    }
}''',
    "stop command test",
)
command.write_text(text, encoding="utf-8")

game_service = Path("crates/ferrum-server/src/game_service.rs")
text = game_service.read_text(encoding="utf-8")
text = text.replace("    use tempfile::tempdir;\n", "")
text = replace_once(
    text,
    "    fn spawn() -> Transform {",
    '''    fn temporary_directory(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rom-game-service-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn spawn() -> Transform {''',
    "temporary directory helper",
)
text = text.replace(
    '''        let directory = tempdir().unwrap();
        let path = directory.path().join("game-state.json");''',
    '''        let directory = temporary_directory("restore");
        let path = directory.join("game-state.json");''',
    1,
)
text = text.replace(
    '''        let directory = tempdir().unwrap();
        let path = directory.path().join("state.json");''',
    '''        let directory = temporary_directory("save-now");
        let path = directory.join("state.json");''',
    1,
)
text = text.replace(
    '''        let directory = tempdir().unwrap();
        let path = directory.path().join("state.json");''',
    '''        let directory = temporary_directory("dimension");
        let path = directory.join("state.json");''',
    1,
)
game_service.write_text(text, encoding="utf-8")
