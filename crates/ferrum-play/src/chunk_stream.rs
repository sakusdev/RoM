use ferrum_world::ChunkPos;

/// Encode Minecraft 26.1.2's clientbound Forget Level Chunk body.
///
/// `FriendlyByteBuf.writeChunkPos` writes Z first and X second as two
/// big-endian signed 32-bit integers.
#[must_use]
pub fn encode_forget_level_chunk(pos: ChunkPos) -> [u8; 8] {
    let mut payload = [0_u8; 8];
    payload[..4].copy_from_slice(&pos.z.to_be_bytes());
    payload[4..].copy_from_slice(&pos.x.to_be_bytes());
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_official_26_1_2_chunk_position_fixture() {
        assert_eq!(
            encode_forget_level_chunk(ChunkPos { x: 12, z: -7 }),
            [0xff, 0xff, 0xff, 0xf9, 0x00, 0x00, 0x00, 0x0c]
        );
    }
}
