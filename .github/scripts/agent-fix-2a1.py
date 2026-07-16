from pathlib import Path
import re


def load(path: str) -> str:
    return Path(path).read_text()


def save(path: str, text: str) -> None:
    Path(path).write_text(text)


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def sub_once(text: str, pattern: str, repl: str, label: str) -> str:
    new, count = re.subn(pattern, repl, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{label}: expected one regex match, found {count}")
    return new

# Replication activation barrier, idempotent entity spawn, bounded backpressure,
# observer death lifecycle, and previous-game-mode wire representation.
path = "crates/ferrum-server/src/game_replication.rs"
text = load(path)
text = sub_once(
    text,
    r"#\[derive\(Debug\)\]\nstruct ReplicationConnection \{.*?\n\}\n\n#\[derive\(Debug\)\]\nenum ReplicationCommand",
    '''#[derive(Debug)]
struct ReplicationConnection {
    endpoint: PlayReaderEndpoint,
    pending: VecDeque<PlayOutput>,
    pending_limit: usize,
    next_teleport_id: i32,
    entities: BTreeMap<PlayerUuid, PlayerEntitySnapshot>,
    active: bool,
    healthy: bool,
    self_initialized: bool,
}

impl ReplicationConnection {
    fn new(endpoint: PlayReaderEndpoint, pending_limit: usize) -> Self {
        Self {
            endpoint,
            pending: VecDeque::new(),
            pending_limit,
            next_teleport_id: 2,
            entities: BTreeMap::new(),
            active: false,
            healthy: true,
            self_initialized: false,
        }
    }

    fn activate(&mut self) -> Result<()> {
        if self.active {
            bail!("replication connection is already active");
        }
        if !self.healthy {
            bail!("replication connection is not healthy");
        }
        self.active = true;
        Ok(())
    }

    fn queue(&mut self, output: PlayOutput, exit: &mut GameReplicationExit) -> bool {
        if !self.active || !self.healthy {
            return false;
        }
        if self.pending.len() >= self.pending_limit {
            self.pending.clear();
            self.entities.clear();
            self.healthy = false;
            exit.dropped_outputs = exit.dropped_outputs.saturating_add(1);
            let _ = self.endpoint.try_disconnect();
            return false;
        }
        self.pending.push_back(output);
        exit.produced_outputs = exit.produced_outputs.saturating_add(1);
        true
    }

    fn queue_teleport(
        &mut self,
        transform: ferrum_game::Transform,
        exit: &mut GameReplicationExit,
    ) -> bool {
        let teleport_id = self.next_teleport_id;
        self.next_teleport_id = self.next_teleport_id.saturating_add(1);
        self.queue(
            PlayOutput::PlayerTeleport {
                teleport_id,
                transform,
            },
            exit,
        )
    }

    fn flush(&mut self, exit: &mut GameReplicationExit) -> bool {
        if !self.healthy {
            return false;
        }
        while let Some(output) = self.pending.pop_front() {
            match self.endpoint.try_submit_output(output) {
                Ok(()) => exit.sent_outputs = exit.sent_outputs.saturating_add(1),
                Err(PlayOutputSubmitError::Full(output)) => {
                    self.pending.push_front(output);
                    return true;
                }
                Err(PlayOutputSubmitError::RuntimeDisconnected(_)) => return false,
            }
        }
        true
    }
}

#[derive(Debug)]
enum ReplicationCommand''',
    "replication connection implementation",
)
text = replace_once(
    text,
    '''    Register {
        uuid: PlayerUuid,
        endpoint: PlayReaderEndpoint,
        reply: SyncSender<Result<(), String>>,
    },
    SyncInventory {''',
    '''    Register {
        uuid: PlayerUuid,
        endpoint: PlayReaderEndpoint,
        reply: SyncSender<Result<(), String>>,
    },
    Activate {
        uuid: PlayerUuid,
        reply: SyncSender<Result<(), String>>,
    },
    SyncInventory {''',
    "activate command enum",
)
text = replace_once(
    text,
    '''    pub fn sync_inventory(&self, uuid: PlayerUuid) -> Result<()> {''',
    '''    pub fn activate(&self, uuid: PlayerUuid) -> Result<()> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(ReplicationCommand::Activate { uuid, reply })
            .context("game replication service is disconnected")?;
        response
            .recv()
            .context("game replication service dropped activation response")?
            .map_err(anyhow::Error::msg)
    }

    pub fn sync_inventory(&self, uuid: PlayerUuid) -> Result<()> {''',
    "activate control method",
)
text = sub_once(
    text,
    r"fn process_commands\(.*?\n\}\n\nfn dispatch_event",
    '''fn process_commands(
    runtime: &SharedGameRuntime,
    commands: &Receiver<ReplicationCommand>,
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<bool> {
    for _ in 0..MAX_COMMANDS_PER_POLL {
        let command = match commands.try_recv() {
            Ok(command) => command,
            Err(TryRecvError::Empty) => return Ok(false),
            Err(TryRecvError::Disconnected) => return Ok(true),
        };
        exit.commands = exit.commands.saturating_add(1);
        match command {
            ReplicationCommand::Register {
                uuid,
                endpoint,
                reply,
            } => {
                let result = match connections.entry(uuid) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(ReplicationConnection::new(
                            endpoint,
                            config.pending_output_limit.get(),
                        ));
                        Ok(())
                    }
                    std::collections::btree_map::Entry::Occupied(_) => Err(format!(
                        "player {uuid:?} is already registered for replication"
                    )),
                };
                let _ = reply.send(result);
            }
            ReplicationCommand::Activate { uuid, reply } => {
                let result = (|| -> Result<()> {
                    let (self_state, snapshots) = runtime
                        .with_state(|state| -> Result<_> {
                            let self_state = match state.player(uuid) {
                                Some(player) if player.connected => Some((
                                    player.vitals,
                                    player_snapshot_from_state(state, uuid)?.with_context(|| {
                                        format!(
                                            "active player {uuid:?} has no authoritative entity snapshot"
                                        )
                                    })?,
                                )),
                                _ => None,
                            };
                            let mut snapshots = Vec::new();
                            for player in state
                                .players()
                                .values()
                                .filter(|player| player.connected && player.uuid != uuid)
                            {
                                snapshots.push(
                                    player_snapshot_from_state(state, player.uuid)?.with_context(
                                        || {
                                            format!(
                                                "online player {:?} has no entity snapshot",
                                                player.uuid
                                            )
                                        },
                                    )?,
                                );
                            }
                            Ok((self_state, snapshots))
                        })
                        .context("cannot read activation snapshot")??;
                    let connection = connections.get_mut(&uuid).with_context(|| {
                        format!("player {uuid:?} is not registered for replication")
                    })?;
                    connection.activate()?;
                    if let Some((vitals, snapshot)) = self_state {
                        queue_set_health(connection, vitals, exit)?;
                        queue_player_info_update(connection, &snapshot, exit)?;
                        connection.self_initialized = true;
                    }
                    if entity_replication_enabled(&config.entity_protocol_ids) {
                        for snapshot in snapshots {
                            queue_player_spawn(connection, snapshot, config, exit)?;
                        }
                    }
                    if !connection.healthy {
                        bail!("initial replication snapshot exceeded the bounded output queue");
                    }
                    Ok(())
                })()
                .map_err(|error| error.to_string());
                if result.is_err() {
                    connections.remove(&uuid);
                }
                let _ = reply.send(result);
            }
            ReplicationCommand::SyncInventory { uuid, reply } => {
                let result = if let Some(connection) = connections.get_mut(&uuid) {
                    if !connection.active {
                        Err(format!("player {uuid:?} replication is not active"))
                    } else {
                        runtime
                            .with_state(|state| {
                                state
                                    .player(uuid)
                                    .map(|player| player.inventory.slots().to_vec())
                            })
                            .map_err(|error| error.to_string())
                            .and_then(|slots| {
                                slots.ok_or_else(|| {
                                    format!(
                                        "player {uuid:?} is missing from authoritative state"
                                    )
                                })
                            })
                            .and_then(|slots| {
                                if slots.len() != PLAYER_INVENTORY_SLOTS {
                                    return Err(format!(
                                        "player inventory has {} slots; expected {PLAYER_INVENTORY_SLOTS}",
                                        slots.len()
                                    ));
                                }
                                if !connection.queue(
                                    PlayOutput::SetContainerContent {
                                        container_id: ferrum_game::PLAYER_CONTAINER_ID,
                                        state_id: 0,
                                        slots,
                                        carried: None,
                                    },
                                    exit,
                                ) {
                                    return Err(
                                        "cannot queue inventory snapshot on an unhealthy replication connection"
                                            .to_owned(),
                                    );
                                }
                                exit.inventory_snapshots =
                                    exit.inventory_snapshots.saturating_add(1);
                                Ok(())
                            })
                    }
                } else {
                    Err(format!("player {uuid:?} is not registered for replication"))
                };
                let _ = reply.send(result);
            }
            ReplicationCommand::Unregister { uuid, reply } => {
                connections.remove(&uuid);
                let _ = reply.send(());
            }
            ReplicationCommand::Shutdown => return Ok(true),
        }
    }
    Ok(false)
}

fn dispatch_event''',
    "process commands",
)
save(path, text)
