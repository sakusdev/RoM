from pathlib import Path

path = Path("crates/ferrum-server/src/main.rs")
source = path.read_text()

old = '''    let online_player = context.state.try_enter_play()?;
    let result = run_static_play_session_with_bridge(
        reader,
        writer,
        config,
        profile,
        session,
        PlayWorldContext {
            shared_world: context.state.world(),
            connection: online_player.connection_id(),
        },
        Some(online_player.play_reader()),
        play_round_limit,
    );
    if let Err(error) = result {
        let reason = format!("Ferrum closed the connection: {error}");
        if let Ok(payload) = encode_play_disconnect(&reason) {
            let _ = write_play_payload(writer, profile, PacketKind::PlayDisconnect, &payload);
            let _ = writer.flush();
        }
        session.disconnect();
        return Err(error);
    }
    Ok(())
}
'''
new = '''    let mut online_player = context.state.try_enter_play()?;
    let writer_worker = match take_live_play_writer() {
        Some(live_writer) => Some(spawn_live_play_writer(
            online_player.take_play_writer()?,
            live_writer,
            profile,
        )?),
        None => None,
    };
    let result = run_static_play_session_with_bridge(
        reader,
        writer,
        config,
        profile,
        session,
        PlayWorldContext {
            shared_world: context.state.world(),
            connection: online_player.connection_id(),
        },
        Some(online_player.play_reader()),
        play_round_limit,
    );
    let writer_result = shutdown_live_play_writer(writer_worker);
    if let Err(error) = result {
        if let Err(writer_error) = writer_result {
            eprintln!("Play writer shutdown also failed: {writer_error:#}");
        }
        let reason = format!("Ferrum closed the connection: {error}");
        if let Ok(payload) = encode_play_disconnect(&reason) {
            let _ = write_play_payload(writer, profile, PacketKind::PlayDisconnect, &payload);
            let _ = writer.flush();
        }
        session.disconnect();
        return Err(error);
    }
    writer_result?;
    Ok(())
}
'''
if old not in source:
    raise SystemExit("Play protocol body marker not found")
path.write_text(source.replace(old, new, 1))
