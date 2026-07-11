from pathlib import Path

path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text(encoding="utf-8")
old = '''    fn enter_play(&self) -> OnlinePlayerGuard<'_> {
        let identity = offline_player_identity("TestPlayer");
        let transform = game_spawn_transform(self.world.world_profile())
            .expect("test spawn transform must be valid");
        self.try_enter_play(&identity, transform)
            .expect("test Play connection must register")
    }'''
new = '''    fn enter_play(&self) -> OnlinePlayerGuard<'_> {
        let connection_number = self.next_connection_id.load(Ordering::Relaxed);
        let identity = offline_player_identity(&format!("Test{connection_number}"));
        let transform = game_spawn_transform(self.world.world_profile())
            .expect("test spawn transform must be valid");
        self.try_enter_play(&identity, transform)
            .expect("test Play connection must register")
    }'''
if old not in text:
    raise SystemExit("test enter_play helper target not found")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
