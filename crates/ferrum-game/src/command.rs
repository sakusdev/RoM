use thiserror::Error;

use crate::{
    Difficulty, EntityPayload, EntityType, GameEvent, GameMode, GameRuleValue, GameState,
    GameStateError, ItemStack, LivingEntityData, PlayerError, PlayerUuid, Transform, Velocity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSource {
    pub name: String,
    pub player: Option<PlayerUuid>,
    pub permission_level: u8,
}

impl CommandSource {
    #[must_use]
    pub fn console() -> Self {
        Self {
            name: "Server".to_owned(),
            player: None,
            permission_level: 4,
        }
    }

    #[must_use]
    pub fn player(name: impl Into<String>, uuid: PlayerUuid, permission_level: u8) -> Self {
        Self {
            name: name.into(),
            player: Some(uuid),
            permission_level,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameCommand {
    Help,
    List,
    Say {
        message: String,
    },
    GameMode {
        mode: GameMode,
        target: Option<String>,
    },
    Teleport {
        target: Option<String>,
        position: [f64; 3],
    },
    Give {
        target: String,
        item: String,
        count: u32,
    },
    TimeSet {
        day_time: i64,
    },
    Difficulty {
        difficulty: Difficulty,
    },
    GameRule {
        name: String,
        value: GameRuleValue,
    },
    Kill {
        target: Option<String>,
    },
    Summon {
        entity_type: String,
        position: [f64; 3],
    },
    SaveAll,
    Stop,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandOutcome {
    pub feedback: String,
    pub events: Vec<GameEvent>,
    pub save_requested: bool,
    pub shutdown_requested: bool,
}

pub fn parse_command(input: &str) -> Result<GameCommand, CommandError> {
    let input = input.trim().strip_prefix('/').unwrap_or(input.trim());
    if input.is_empty() {
        return Err(CommandError::EmptyCommand);
    }
    let parts = input.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["help"] | ["?"] => Ok(GameCommand::Help),
        ["list"] => Ok(GameCommand::List),
        ["say", message @ ..] if !message.is_empty() => Ok(GameCommand::Say {
            message: message.join(" "),
        }),
        ["gamemode", mode] => Ok(GameCommand::GameMode {
            mode: GameMode::parse(mode)?,
            target: None,
        }),
        ["gamemode", mode, target] => Ok(GameCommand::GameMode {
            mode: GameMode::parse(mode)?,
            target: Some((*target).to_owned()),
        }),
        ["tp", x, y, z] => Ok(GameCommand::Teleport {
            target: None,
            position: [parse_f64(x)?, parse_f64(y)?, parse_f64(z)?],
        }),
        ["tp", target, x, y, z] => Ok(GameCommand::Teleport {
            target: Some((*target).to_owned()),
            position: [parse_f64(x)?, parse_f64(y)?, parse_f64(z)?],
        }),
        ["give", target, item] => Ok(GameCommand::Give {
            target: (*target).to_owned(),
            item: (*item).to_owned(),
            count: 1,
        }),
        ["give", target, item, count] => Ok(GameCommand::Give {
            target: (*target).to_owned(),
            item: (*item).to_owned(),
            count: parse_u32(count)?,
        }),
        ["time", "set", value] => Ok(GameCommand::TimeSet {
            day_time: parse_day_time(value)?,
        }),
        ["difficulty", value] => Ok(GameCommand::Difficulty {
            difficulty: parse_difficulty(value)?,
        }),
        ["gamerule", name, value] => Ok(GameCommand::GameRule {
            name: (*name).to_owned(),
            value: parse_game_rule_value(value)?,
        }),
        ["kill"] => Ok(GameCommand::Kill { target: None }),
        ["kill", target] => Ok(GameCommand::Kill {
            target: Some((*target).to_owned()),
        }),
        ["summon", entity_type, x, y, z] => Ok(GameCommand::Summon {
            entity_type: (*entity_type).to_owned(),
            position: [parse_f64(x)?, parse_f64(y)?, parse_f64(z)?],
        }),
        ["save-all"] | ["save-all", "flush"] => Ok(GameCommand::SaveAll),
        ["stop"] | ["exit"] | ["quit"] => Ok(GameCommand::Stop),
        _ => Err(CommandError::InvalidSyntax {
            input: input.to_owned(),
        }),
    }
}

pub fn execute_command(
    state: &mut GameState,
    source: &CommandSource,
    input: &str,
) -> Result<CommandOutcome, CommandError> {
    let command = parse_command(input)?;
    execute_parsed_command(state, source, command)
}

pub fn execute_parsed_command(
    state: &mut GameState,
    source: &CommandSource,
    command: GameCommand,
) -> Result<CommandOutcome, CommandError> {
    match command {
        GameCommand::Help => {
            require_permission(source, 0)?;
            Ok(CommandOutcome {
        feedback: "Commands: help, list, say <message>, gamemode <mode> [player], tp [player] <x> <y> <z>, give <player> <item> [count], time set <value>, difficulty <value>, gamerule <name> <value>, kill [player], summon zombie <x> <y> <z>, save-all [flush], stop (aliases: exit, quit)".to_owned(),
        events: Vec::new(),
        save_requested: false,
        shutdown_requested: false,
    })
        }
        GameCommand::List => {
            require_permission(source, 0)?;
            let names = state
                .players()
                .values()
                .filter(|player| player.connected)
                .map(|player| player.name.as_str())
                .collect::<Vec<_>>();
            Ok(CommandOutcome {
                feedback: format!(
                    "There are {} player(s) online: {}",
                    names.len(),
                    names.join(", ")
                ),
                events: Vec::new(),
                save_requested: false,
                shutdown_requested: false,
            })
        }
        GameCommand::Say { message } => {
            require_permission(source, 1)?;
            let rendered = format!("[{}] {message}", source.name);
            Ok(CommandOutcome {
                feedback: rendered.clone(),
                events: vec![GameEvent::Broadcast { message: rendered }],
                save_requested: false,
                shutdown_requested: false,
            })
        }
        GameCommand::GameMode { mode, target } => {
            require_permission(source, 2)?;
            let target = resolve_target(state, source, target.as_deref())?;
            let events = state.set_game_mode(target, mode)?;
            let name = state
                .player(target)
                .expect("resolved player exists")
                .name
                .clone();
            Ok(CommandOutcome {
                feedback: format!("Set {name}'s game mode to {mode:?}"),
                events,
                save_requested: false,
                shutdown_requested: false,
            })
        }
        GameCommand::Teleport { target, position } => {
            require_permission(source, 2)?;
            let target = resolve_target(state, source, target.as_deref())?;
            let current = player_transform(state, target)?;
            let transform = Transform::new(position, current.yaw, current.pitch, false)?;
            let events = state.teleport_player(target, transform)?;
            let name = state
                .player(target)
                .expect("resolved player exists")
                .name
                .clone();
            Ok(CommandOutcome {
                feedback: format!(
                    "Teleported {name} to {:.3} {:.3} {:.3}",
                    position[0], position[1], position[2]
                ),
                events,
                save_requested: false,
                shutdown_requested: false,
            })
        }
        GameCommand::Give {
            target,
            item,
            count,
        } => {
            require_permission(source, 2)?;
            if count == 0 || count > 6_400 {
                return Err(CommandError::InvalidGiveCount { count });
            }
            let target_uuid = resolve_target(state, source, Some(&target))?;
            let mut remaining = count;
            let mut inserted = 0_u32;
            let mut events = Vec::new();
            while remaining > 0 {
                let batch = remaining.min(crate::MAX_VANILLA_STACK_SIZE);
                let stack = ItemStack::new(item.clone(), batch)?;
                let (remainder, batch_events) = state.give_item(target_uuid, stack)?;
                let not_inserted = remainder.as_ref().map_or(0, ItemStack::count);
                let accepted = batch - not_inserted;
                inserted += accepted;
                events.extend(batch_events);
                remaining -= batch;
                if not_inserted > 0 {
                    break;
                }
            }
            let name = state
                .player(target_uuid)
                .expect("resolved player exists")
                .name
                .clone();
            Ok(CommandOutcome {
                feedback: format!("Gave {inserted} {item} to {name}"),
                events,
                save_requested: false,
                shutdown_requested: false,
            })
        }
        GameCommand::TimeSet { day_time } => {
            require_permission(source, 2)?;
            let events = state.set_day_time(day_time);
            Ok(CommandOutcome {
                feedback: format!("Set the time to {day_time}"),
                events,
                save_requested: false,
                shutdown_requested: false,
            })
        }
        GameCommand::Difficulty { difficulty } => {
            require_permission(source, 2)?;
            state.set_difficulty(difficulty);
            Ok(CommandOutcome {
                feedback: format!("Set difficulty to {difficulty:?}"),
                events: Vec::new(),
                save_requested: false,
                shutdown_requested: false,
            })
        }
        GameCommand::GameRule { name, value } => {
            require_permission(source, 2)?;
            state.set_game_rule(name.clone(), value.clone());
            Ok(CommandOutcome {
                feedback: format!("Set game rule {name} to {value:?}"),
                events: Vec::new(),
                save_requested: false,
                shutdown_requested: false,
            })
        }
        GameCommand::Kill { target } => {
            require_permission(source, 2)?;
            let target = resolve_target(state, source, target.as_deref())?;
            let events = state.kill_player(target)?;
            let name = state
                .player(target)
                .expect("resolved player exists")
                .name
                .clone();
            Ok(CommandOutcome {
                feedback: format!("Killed {name}"),
                events,
                save_requested: false,
                shutdown_requested: false,
            })
        }
        GameCommand::Summon {
            entity_type,
            position,
        } => {
            require_permission(source, 2)?;
            let entity_type = if entity_type.contains(':') {
                entity_type
            } else {
                format!("minecraft:{entity_type}")
            };
            if entity_type != "minecraft:zombie" {
                return Err(CommandError::UnsupportedSummonEntity { entity_type });
            }
            let entity_type = EntityType::new(entity_type)?;
            let transform = Transform::new(position, 0.0, 0.0, true)?;
            let living = LivingEntityData::new(20.0)?
                .with_drops(vec![ItemStack::new("minecraft:rotten_flesh", 1)?])?
                .with_hostile_ai();
            let events = state.spawn_entity(
                entity_type,
                transform,
                Velocity::default(),
                EntityPayload::Living(living),
            )?;
            Ok(CommandOutcome {
                feedback: format!(
                    "Summoned minecraft:zombie at {:.3} {:.3} {:.3}",
                    position[0], position[1], position[2]
                ),
                events,
                save_requested: false,
                shutdown_requested: false,
            })
        }
        GameCommand::SaveAll => {
            require_permission(source, 4)?;
            Ok(CommandOutcome {
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
}

fn require_permission(source: &CommandSource, required: u8) -> Result<(), CommandError> {
    if source.permission_level < required {
        return Err(CommandError::PermissionDenied {
            required,
            actual: source.permission_level,
        });
    }
    Ok(())
}

fn resolve_target(
    state: &GameState,
    source: &CommandSource,
    target: Option<&str>,
) -> Result<PlayerUuid, CommandError> {
    if let Some(target) = target {
        return state
            .player_uuid_by_name(target)
            .ok_or_else(|| CommandError::UnknownPlayer {
                name: target.to_owned(),
            });
    }
    source.player.ok_or(CommandError::TargetRequired)
}

fn player_transform(state: &GameState, uuid: PlayerUuid) -> Result<Transform, CommandError> {
    let player = state
        .player(uuid)
        .ok_or(GameStateError::UnknownPlayer { uuid })?;
    let entity_id = player
        .entity_id
        .ok_or(GameStateError::PlayerMissingEntity { uuid })?;
    Ok(state
        .entities()
        .get(entity_id)
        .ok_or(GameStateError::PlayerMissingEntity { uuid })?
        .transform)
}

fn parse_f64(value: &str) -> Result<f64, CommandError> {
    value.parse().map_err(|_| CommandError::InvalidNumber {
        value: value.to_owned(),
    })
}

fn parse_u32(value: &str) -> Result<u32, CommandError> {
    value.parse().map_err(|_| CommandError::InvalidNumber {
        value: value.to_owned(),
    })
}

fn parse_day_time(value: &str) -> Result<i64, CommandError> {
    match value {
        "day" => Ok(1_000),
        "noon" => Ok(6_000),
        "night" => Ok(13_000),
        "midnight" => Ok(18_000),
        _ => value.parse().map_err(|_| CommandError::InvalidNumber {
            value: value.to_owned(),
        }),
    }
}

fn parse_difficulty(value: &str) -> Result<Difficulty, CommandError> {
    match value {
        "peaceful" | "p" | "0" => Ok(Difficulty::Peaceful),
        "easy" | "e" | "1" => Ok(Difficulty::Easy),
        "normal" | "n" | "2" => Ok(Difficulty::Normal),
        "hard" | "h" | "3" => Ok(Difficulty::Hard),
        _ => Err(CommandError::UnknownDifficulty {
            value: value.to_owned(),
        }),
    }
}

fn parse_game_rule_value(value: &str) -> Result<GameRuleValue, CommandError> {
    match value {
        "true" => Ok(GameRuleValue::Boolean(true)),
        "false" => Ok(GameRuleValue::Boolean(false)),
        _ => value
            .parse::<i32>()
            .map(GameRuleValue::Integer)
            .map_err(|_| CommandError::InvalidGameRuleValue {
                value: value.to_owned(),
            }),
    }
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error(transparent)]
    State(#[from] GameStateError),
    #[error(transparent)]
    Player(#[from] PlayerError),
    #[error(transparent)]
    Inventory(#[from] crate::InventoryError),
    #[error(transparent)]
    Entity(#[from] crate::EntityError),
    #[error("command is empty")]
    EmptyCommand,
    #[error("invalid command syntax: {input}; use help to list commands")]
    InvalidSyntax { input: String },
    #[error("invalid numeric value {value}")]
    InvalidNumber { value: String },
    #[error("permission level {required} is required; source has {actual}")]
    PermissionDenied { required: u8, actual: u8 },
    #[error("a player target is required for the console")]
    TargetRequired,
    #[error("unknown player {name}")]
    UnknownPlayer { name: String },
    #[error("give count {count} must be between 1 and 6400")]
    InvalidGiveCount { count: u32 },
    #[error("unknown difficulty {value}")]
    UnknownDifficulty { value: String },
    #[error("invalid game rule value {value}")]
    InvalidGameRuleValue { value: String },
    #[error("summoning entity type {entity_type} is not implemented")]
    UnsupportedSummonEntity { entity_type: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    fn state_with_player() -> (GameState, PlayerUuid) {
        let mut state = GameState::default();
        let uuid = PlayerUuid::new(1);
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        (state, uuid)
    }

    #[test]
    fn parses_supported_vanilla_commands() {
        assert_eq!(parse_command("help").unwrap(), GameCommand::Help);
        assert_eq!(parse_command("/list").unwrap(), GameCommand::List);
        assert_eq!(
            parse_command("gamemode creative Steve").unwrap(),
            GameCommand::GameMode {
                mode: GameMode::Creative,
                target: Some("Steve".to_owned())
            }
        );
        assert_eq!(
            parse_command("time set night").unwrap(),
            GameCommand::TimeSet { day_time: 13_000 }
        );
        assert_eq!(
            parse_command("summon zombie 4 65 -2").unwrap(),
            GameCommand::Summon {
                entity_type: "zombie".to_owned(),
                position: [4.0, 65.0, -2.0],
            }
        );
        assert!(parse_command("unknown command").is_err());
    }

    #[test]
    fn executes_gameplay_commands_against_authoritative_state() {
        let (mut state, uuid) = state_with_player();
        let source = CommandSource::console();
        execute_command(&mut state, &source, "/gamemode creative Steve").unwrap();
        execute_command(&mut state, &source, "/give Steve minecraft:stone 80").unwrap();
        execute_command(&mut state, &source, "/tp Steve 10 70 -4").unwrap();
        assert_eq!(state.player(uuid).unwrap().game_mode, GameMode::Creative);
        assert_eq!(state.player(uuid).unwrap().inventory.occupied_slots(), 2);
        let entity = state
            .entities()
            .get(state.player(uuid).unwrap().entity_id.unwrap())
            .unwrap();
        assert_eq!(entity.transform.position, [10.0, 70.0, -4.0]);
    }

    #[test]
    fn summons_the_first_supported_hostile_mob() {
        let (mut state, _) = state_with_player();
        let outcome = execute_command(
            &mut state,
            &CommandSource::console(),
            "/summon zombie 8 65 0",
        )
        .unwrap();
        let entity = match &outcome.events[0] {
            GameEvent::EntitySpawned { entity } => entity,
            event => panic!("unexpected event: {event:?}"),
        };
        assert_eq!(entity.entity_type.as_str(), "minecraft:zombie");
        assert!(entity.living().unwrap().ai.is_some());
        assert!(matches!(
            execute_command(
                &mut state,
                &CommandSource::console(),
                "/summon cow 8 65 0",
            ),
            Err(CommandError::UnsupportedSummonEntity { .. })
        ));
    }

    #[test]
    fn enforces_permission_levels_and_self_targets() {
        let (mut state, uuid) = state_with_player();
        let player = CommandSource::player("Steve", uuid, 0);
        assert!(execute_command(&mut state, &player, "/list").is_ok());
        assert!(matches!(
            execute_command(&mut state, &player, "/gamemode creative"),
            Err(CommandError::PermissionDenied { .. })
        ));
        let operator = CommandSource::player("Steve", uuid, 2);
        execute_command(&mut state, &operator, "/kill").unwrap();
        assert!(state.player(uuid).unwrap().vitals.is_dead());
    }

    #[test]
    fn save_all_requests_persistence() {
        let mut state = GameState::default();
        let outcome = execute_command(&mut state, &CommandSource::console(), "/save-all").unwrap();
        assert!(outcome.save_requested);
        assert!(!outcome.shutdown_requested);
        assert_eq!(outcome.events, [GameEvent::SaveRequested]);
    }

    #[test]
    fn help_lists_commands_and_exit_alias_stops() {
        let mut state = GameState::default();
        let source = CommandSource::console();
        let help = execute_command(&mut state, &source, "help").unwrap();
        assert!(help.feedback.contains("say <message>"));
        assert!(!help.shutdown_requested);
        for alias in ["exit", "quit"] {
            let outcome = execute_command(&mut state, &source, alias).unwrap();
            assert!(outcome.save_requested);
            assert!(outcome.shutdown_requested);
        }
    }

    #[test]
    fn stop_requests_save_and_shutdown() {
        let mut state = GameState::default();
        let outcome = execute_command(&mut state, &CommandSource::console(), "/stop").unwrap();
        assert!(outcome.save_requested);
        assert!(outcome.shutdown_requested);
        assert_eq!(outcome.events, [GameEvent::ShutdownRequested]);
    }
}
