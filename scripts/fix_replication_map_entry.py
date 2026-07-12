from pathlib import Path

path = Path("crates/ferrum-server/src/game_replication.rs")
text = path.read_text(encoding="utf-8")
old = '''                let result = if connections.contains_key(&uuid) {
                    Err(format!(
                        "player {uuid:?} is already registered for replication"
                    ))
                } else {
                    connections.insert(uuid, ReplicationConnection::new(endpoint, pending_limit));
                    Ok(())
                };'''
new = '''                let result = match connections.entry(uuid) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(ReplicationConnection::new(endpoint, pending_limit));
                        Ok(())
                    }
                    std::collections::btree_map::Entry::Occupied(_) => Err(format!(
                        "player {uuid:?} is already registered for replication"
                    )),
                };'''
if old not in text:
    raise SystemExit("replication registration target not found")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
