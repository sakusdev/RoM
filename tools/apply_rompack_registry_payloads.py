from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


main = "crates/ferrum-server/src/main.rs"

replace_once(
    main,
    "use ferrum_configuration::{\n"
    "    KnownPack, KnownPackDecodeLimits, decode_client_information, decode_known_packs,\n"
    "    encode_feature_flags, encode_known_packs, encode_registry_data, encode_tags,\n"
    "};\n",
    "use ferrum_configuration::{\n"
    "    KnownPack, KnownPackDecodeLimits, RegistryData, RegistryEntry,\n"
    "    decode_client_information, decode_known_packs, encode_feature_flags, encode_known_packs,\n"
    "    encode_registry_data, encode_tags,\n"
    "};\n",
)
replace_once(
    main,
    "use ferrum_rompack::{RomPack, RomPackPacket, RomPackWorld, read_rompack};\n",
    "use ferrum_rompack::{RomPack, RomPackPacket, RomPackRegistry, RomPackWorld, read_rompack};\n",
)
replace_once(
    main,
    "struct ServerState {\n"
    "    online_players: AtomicI32,\n"
    "    next_connection_id: AtomicU64,\n"
    "    world: play_runtime::SharedWorld,\n"
    "}\n",
    "struct ServerState {\n"
    "    online_players: AtomicI32,\n"
    "    next_connection_id: AtomicU64,\n"
    "    world: play_runtime::SharedWorld,\n"
    "    registry_payloads: Vec<Vec<u8>>,\n"
    "}\n",
)
replace_once(
    main,
    "    fn with_world(initial_online_players: i32, world: RomPackWorld) -> Result<Self> {\n"
    "        Ok(Self {\n"
    "            online_players: AtomicI32::new(initial_online_players),\n"
    "            next_connection_id: AtomicU64::new(1),\n"
    "            world: play_runtime::SharedWorld::new(\n"
    "                ChunkPos {\n"
    "                    x: STATIC_CHUNK_X,\n"
    "                    z: STATIC_CHUNK_Z,\n"
    "                },\n"
    "                world,\n"
    "            )?,\n"
    "        })\n"
    "    }\n",
    "    #[cfg(test)]\n"
    "    fn with_world(initial_online_players: i32, world: RomPackWorld) -> Result<Self> {\n"
    "        Self::with_runtime(\n"
    "            initial_online_players,\n"
    "            world,\n"
    "            builtin_26_1_2_registry_payloads()?,\n"
    "        )\n"
    "    }\n\n"
    "    fn with_runtime(\n"
    "        initial_online_players: i32,\n"
    "        world: RomPackWorld,\n"
    "        registry_payloads: Vec<Vec<u8>>,\n"
    "    ) -> Result<Self> {\n"
    "        Ok(Self {\n"
    "            online_players: AtomicI32::new(initial_online_players),\n"
    "            next_connection_id: AtomicU64::new(1),\n"
    "            world: play_runtime::SharedWorld::new(\n"
    "                ChunkPos {\n"
    "                    x: STATIC_CHUNK_X,\n"
    "                    z: STATIC_CHUNK_Z,\n"
    "                },\n"
    "                world,\n"
    "            )?,\n"
    "            registry_payloads,\n"
    "        })\n"
    "    }\n",
)
replace_once(
    main,
    "    fn world(&self) -> &play_runtime::SharedWorld {\n"
    "        &self.world\n"
    "    }\n",
    "    fn world(&self) -> &play_runtime::SharedWorld {\n"
    "        &self.world\n"
    "    }\n\n"
    "    fn registry_payloads(&self) -> &[Vec<u8>] {\n"
    "        &self.registry_payloads\n"
    "    }\n",
)
replace_once(
    main,
    "    let (runtime_profile, world_profile) = if let Some(version_pack) = &cli.version_pack {\n"
    "        let loaded = load_version_pack(version_pack, &config)?;\n"
    "        (loaded.profile, loaded.world)\n"
    "    } else {\n"
    "        (\n"
    "            config\n"
    "                .protocol_profile()\n"
    "                .context(\"cannot build configured protocol profile\")?,\n"
    "            play_runtime::builtin_world_profile(),\n"
    "        )\n"
    "    };\n"
    "    config.runtime_profile = Some(runtime_profile);\n"
    "    let state = Arc::new(ServerState::with_world(\n"
    "        config.online_players,\n"
    "        world_profile,\n"
    "    )?);\n",
    "    let (runtime_profile, world_profile, registry_payloads) =\n"
    "        if let Some(version_pack) = &cli.version_pack {\n"
    "            let loaded = load_version_pack(version_pack, &config)?;\n"
    "            (loaded.profile, loaded.world, loaded.registry_payloads)\n"
    "        } else {\n"
    "            let registry_payloads =\n"
    "                if config.profile_name.as_deref() == Some(version_26_1_2::PROFILE_NAME) {\n"
    "                    builtin_26_1_2_registry_payloads()?\n"
    "                } else {\n"
    "                    Vec::new()\n"
    "                };\n"
    "            (\n"
    "                config\n"
    "                    .protocol_profile()\n"
    "                    .context(\"cannot build configured protocol profile\")?,\n"
    "                play_runtime::builtin_world_profile(),\n"
    "                registry_payloads,\n"
    "            )\n"
    "        };\n"
    "    config.runtime_profile = Some(runtime_profile);\n"
    "    let state = Arc::new(ServerState::with_runtime(\n"
    "        config.online_players,\n"
    "        world_profile,\n"
    "        registry_payloads,\n"
    "    )?);\n",
)
replace_once(
    main,
    "struct LoadedVersionPack {\n"
    "    profile: ProtocolProfile,\n"
    "    world: RomPackWorld,\n"
    "}\n",
    "struct LoadedVersionPack {\n"
    "    profile: ProtocolProfile,\n"
    "    world: RomPackWorld,\n"
    "    registry_payloads: Vec<Vec<u8>>,\n"
    "}\n",
)
replace_once(
    main,
    "    let profile =\n"
    "        protocol_profile_from_packets(&config.version_name, pack.metadata.protocol, &pack.packets)?;\n"
    "    println!(\n",
    "    let profile =\n"
    "        protocol_profile_from_packets(&config.version_name, pack.metadata.protocol, &pack.packets)?;\n"
    "    let registry_payloads = registry_payloads_from_pack(&pack.registries)?;\n"
    "    println!(\n",
)
replace_once(
    main,
    "    Ok(LoadedVersionPack {\n"
    "        profile,\n"
    "        world: pack.world,\n"
    "    })\n"
    "}\n\n"
    "fn validate_world_profile(world: &RomPackWorld) -> Result<()> {\n",
    "    Ok(LoadedVersionPack {\n"
    "        profile,\n"
    "        world: pack.world,\n"
    "        registry_payloads,\n"
    "    })\n"
    "}\n\n"
    "fn registry_payloads_from_pack(registries: &[RomPackRegistry]) -> Result<Vec<Vec<u8>>> {\n"
    "    let runtime_registries = registries\n"
    "        .iter()\n"
    "        .map(|registry| {\n"
    "            RegistryData::new(\n"
    "                registry.id.clone(),\n"
    "                registry\n"
    "                    .entries\n"
    "                    .iter()\n"
    "                    .cloned()\n"
    "                    .map(|entry| RegistryEntry::new(entry, None))\n"
    "                    .collect(),\n"
    "            )\n"
    "        })\n"
    "        .collect();\n"
    "    encode_registry_payloads(runtime_registries)\n"
    "}\n\n"
    "fn builtin_26_1_2_registry_payloads() -> Result<Vec<Vec<u8>>> {\n"
    "    encode_registry_payloads(version_26_1_2::configuration_registries())\n"
    "}\n\n"
    "fn encode_registry_payloads(registries: Vec<RegistryData>) -> Result<Vec<Vec<u8>>> {\n"
    "    registries\n"
    "        .into_iter()\n"
    "        .map(|registry| {\n"
    "            let id = registry.id.clone();\n"
    "            encode_registry_data(&registry)\n"
    "                .with_context(|| format!(\"cannot encode generated registry {id}\"))\n"
    "        })\n"
    "        .collect()\n"
    "}\n\n"
    "fn validate_world_profile(world: &RomPackWorld) -> Result<()> {\n",
)
replace_once(
    main,
    "    send_registry_data(writer, config, profile)?;\n",
    "    send_registry_data(writer, context.state.registry_payloads(), profile)?;\n",
)
replace_once(
    main,
    "fn send_registry_data<W: Write>(\n"
    "    writer: &mut W,\n"
    "    config: &ServerConfig,\n"
    "    profile: &ProtocolProfile,\n"
    ") -> Result<()> {\n"
    "    if config.profile_name.as_deref() != Some(version_26_1_2::PROFILE_NAME) {\n"
    "        return Ok(());\n"
    "    }\n\n"
    "    let packet_id = profile.packets().require(PacketKind::RegistryData)?;\n"
    "    for registry in version_26_1_2::configuration_registries() {\n"
    "        let body = encode_registry_data(&registry)?;\n"
    "        write_packet(\n"
    "            writer,\n"
    "            &build_packet(packet_id, |output| {\n"
    "                output.extend_from_slice(&body);\n"
    "                Ok(())\n"
    "            })?,\n"
    "        )?;\n"
    "    }\n"
    "    Ok(())\n"
    "}\n",
    "fn send_registry_data<W: Write>(\n"
    "    writer: &mut W,\n"
    "    registry_payloads: &[Vec<u8>],\n"
    "    profile: &ProtocolProfile,\n"
    ") -> Result<()> {\n"
    "    if registry_payloads.is_empty() {\n"
    "        return Ok(());\n"
    "    }\n\n"
    "    let packet_id = profile.packets().require(PacketKind::RegistryData)?;\n"
    "    for body in registry_payloads {\n"
    "        write_packet(\n"
    "            writer,\n"
    "            &build_packet(packet_id, |output| {\n"
    "                output.extend_from_slice(body);\n"
    "                Ok(())\n"
    "            })?,\n"
    "        )?;\n"
    "    }\n"
    "    Ok(())\n"
    "}\n",
)
replace_once(
    main,
    "    #[test]\n    fn parses_expected_config_argument() {\n",
    "    #[test]\n"
    "    fn generated_registry_manifest_drives_configuration_payloads() {\n"
    "        let registries = vec![RomPackRegistry {\n"
    "            id: \"minecraft:test_registry\".to_owned(),\n"
    "            entries: vec![\n"
    "                \"minecraft:alpha\".to_owned(),\n"
    "                \"minecraft:beta\".to_owned(),\n"
    "            ],\n"
    "        }];\n"
    "        let payloads = registry_payloads_from_pack(&registries).unwrap();\n"
    "        let expected = encode_registry_data(&RegistryData::new(\n"
    "            \"minecraft:test_registry\",\n"
    "            vec![\n"
    "                RegistryEntry::new(\"minecraft:alpha\", None),\n"
    "                RegistryEntry::new(\"minecraft:beta\", None),\n"
    "            ],\n"
    "        ))\n"
    "        .unwrap();\n"
    "        assert_eq!(payloads, vec![expected]);\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn parses_expected_config_argument() {\n",
)

replace_once(
    "README.md",
    "7. A deterministic, integrity-protected `.rompack` records registry IDs, source-resource hashes, source hashes, and patch-set identity.\n"
    "8. The native `rom-server` validates that pack against its built-in 26.1.2 profile before starting.\n",
    "7. A deterministic, integrity-protected `.rompack` records registry IDs, source-resource hashes, source hashes, and patch-set identity.\n"
    "8. The native `ferrum-server` validates that pack against its built-in 26.1.2 generation profile before starting.\n",
)
replace_once(
    "README.md",
    "- Packet IDs, world height, flat-world block-state IDs, and biome ID loaded from the generated schema-v3 `.rompack` during Bootstrap startup\n"
    "- All 28 synchronized 26.1.2 registries with 382 vanilla entries\n",
    "- Packet IDs, world height, flat-world block-state IDs, biome ID, and Configuration registry payloads loaded from the generated schema-v3 `.rompack` during Bootstrap startup\n"
    "- All 28 synchronized 26.1.2 registries with 382 vanilla entries\n",
)
replace_once(
    "README.md",
    "- Runtime replacement of remaining dimension registry payloads and other gameplay constants with generated pack data\n",
    "- Runtime replacement of remaining gameplay constants with generated pack data\n",
)
replace_once(
    "README.md",
    "1. Move remaining dimension registry payloads and gameplay constants into generated packs\n",
    "1. Move remaining gameplay constants into generated packs\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "12. Revalidate the pack before `rom-bootstrap run`; the native server then builds its `ProtocolProfile` and initial shared world from pack metadata.\n",
    "12. Revalidate the pack before `rom-bootstrap run`; the native server then builds its `ProtocolProfile`, Configuration registry payloads, and initial shared world from pack metadata.\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "1. Move remaining dimension registry payloads and gameplay constants into generated packs.\n",
    "1. Move remaining gameplay constants into generated packs.\n",
)
