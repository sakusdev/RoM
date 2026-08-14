//! Generated Minecraft Java Edition 26.1.2 network tag manifest.
//!
//! Source tag JSON and registry IDs come from the SHA-1-verified official
//! 26.1.2 server artifact. Integer entries are the protocol registry IDs
//! consumed by `ClientboundUpdateTagsPacket`.

use rom_configuration::{TagEntry, TagRegistry};

pub const TAG_REGISTRY_COUNT: usize = 20;
pub const TAG_COUNT: usize = 758;
pub const TAG_ENTRY_COUNT: usize = 8196;
pub const TAG_MANIFEST_SHA256: &str =
    "8f4daa55eff566bc09a5d6626aea9e32b557abdd2283500e5c2fa79a7f82d49d";

#[must_use]
pub fn configuration_tags() -> Vec<TagRegistry> {
    vec![
        TagRegistry::new(
            "minecraft:banner_pattern",
            vec![
                TagEntry::new(
                    "minecraft:no_item_required",
                    vec![
                        26, 27, 28, 29, 31, 38, 35, 37, 32, 36, 34, 33, 25, 5, 30, 39, 40, 41, 42,
                        7, 10, 9, 8, 3, 23, 19, 17, 20, 18, 1, 14, 15,
                    ],
                ),
                TagEntry::new("minecraft:pattern_item/bordure_indented", vec![6]),
                TagEntry::new("minecraft:pattern_item/creeper", vec![4]),
                TagEntry::new("minecraft:pattern_item/field_masoned", vec![2]),
                TagEntry::new("minecraft:pattern_item/flow", vec![11]),
                TagEntry::new("minecraft:pattern_item/flower", vec![12]),
                TagEntry::new("minecraft:pattern_item/globe", vec![13]),
                TagEntry::new("minecraft:pattern_item/guster", vec![16]),
                TagEntry::new("minecraft:pattern_item/mojang", vec![21]),
                TagEntry::new("minecraft:pattern_item/piglin", vec![22]),
                TagEntry::new("minecraft:pattern_item/skull", vec![24]),
            ],
        ),
        TagRegistry::new(
            "minecraft:block",
            vec![
                TagEntry::new("minecraft:acacia_logs", vec![53, 75, 64, 83]),
                TagEntry::new("minecraft:air", vec![0, 794, 795]),
                TagEntry::new(
                    "minecraft:all_hanging_signs",
                    vec![
                        234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248,
                        249, 250, 251, 252, 253, 255, 256, 254, 257,
                    ],
                ),
                TagEntry::new(
                    "minecraft:all_signs",
                    vec![
                        210, 211, 212, 213, 215, 216, 217, 901, 902, 218, 219, 214, 224, 225, 226,
                        227, 229, 230, 231, 903, 904, 232, 233, 228, 234, 235, 236, 237, 238, 239,
                        240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 255,
                        256, 254, 257,
                    ],
                ),
                TagEntry::new(
                    "minecraft:ancient_city_replaceable",
                    vec![
                        1123, 1136, 1132, 1138, 1134, 1137, 1135, 1139, 1124, 1141, 1142, 147,
                    ],
                ),
                TagEntry::new("minecraft:animals_spawnable_on", vec![8]),
                TagEntry::new("minecraft:anvil", vec![467, 468, 469]),
                TagEntry::new(
                    "minecraft:armadillo_spawnable_on",
                    vec![8, 554, 484, 488, 485, 498, 496, 492, 39, 10],
                ),
                TagEntry::new("minecraft:axolotls_spawnable_on", vec![281]),
                TagEntry::new(
                    "minecraft:azalea_grows_on",
                    vec![
                        9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 37, 39, 38, 554, 484, 485,
                        486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498, 499, 278,
                        1000,
                    ],
                ),
                TagEntry::new(
                    "minecraft:azalea_root_replaceable",
                    vec![
                        1, 2, 4, 6, 984, 1123, 9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 554,
                        484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498,
                        499, 39, 281, 40, 37, 278, 1000,
                    ],
                ),
                TagEntry::new(
                    "minecraft:badlands_terracotta",
                    vec![554, 484, 488, 485, 498, 496, 492],
                ),
                TagEntry::new("minecraft:bamboo_blocks", vec![60, 70]),
                TagEntry::new(
                    "minecraft:banners",
                    vec![
                        563, 564, 565, 566, 567, 568, 569, 570, 571, 572, 573, 574, 575, 576, 577,
                        578, 579, 580, 581, 582, 583, 584, 585, 586, 587, 588, 589, 590, 591, 592,
                        593, 594,
                    ],
                ),
                TagEntry::new(
                    "minecraft:bars",
                    vec![341, 342, 346, 343, 347, 344, 348, 345, 349],
                ),
                TagEntry::new("minecraft:base_stone_nether", vec![285, 288, 924]),
                TagEntry::new(
                    "minecraft:base_stone_overworld",
                    vec![1, 2, 4, 6, 984, 1123],
                ),
                TagEntry::new("minecraft:bats_spawnable_on", vec![1, 2, 4, 6, 984, 1123]),
                TagEntry::new(
                    "minecraft:beacon_base_blocks",
                    vec![915, 403, 205, 174, 175],
                ),
                TagEntry::new(
                    "minecraft:beds",
                    vec![
                        124, 125, 121, 122, 119, 117, 123, 113, 118, 115, 112, 111, 116, 120, 110,
                        114,
                    ],
                ),
                TagEntry::new(
                    "minecraft:bee_attractive",
                    vec![
                        157, 1163, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 170, 159,
                        557, 558, 560, 559, 664, 98, 1111, 33, 93, 1113, 1114, 657, 1109, 280,
                    ],
                ),
                TagEntry::new(
                    "minecraft:bee_growables",
                    vec![665, 441, 442, 207, 365, 364, 662, 663, 861, 1107, 1108],
                ),
                TagEntry::new("minecraft:beehives", vec![911, 912]),
                TagEntry::new(
                    "minecraft:beneath_bamboo_podzol_replaceable",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373],
                ),
                TagEntry::new(
                    "minecraft:beneath_tree_podzol_replaceable",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373],
                ),
                TagEntry::new("minecraft:birch_logs", vec![51, 73, 62, 81]),
                TagEntry::new("minecraft:blocks_wind_charge_explosions", vec![524, 34]),
                TagEntry::new(
                    "minecraft:buttons",
                    vec![
                        443, 444, 445, 446, 447, 449, 450, 897, 898, 451, 452, 448, 275, 939,
                    ],
                ),
                TagEntry::new(
                    "minecraft:camel_sand_step_sound_blocks",
                    vec![
                        37, 39, 38, 726, 727, 728, 729, 730, 731, 732, 733, 734, 735, 736, 737,
                        738, 739, 740, 741,
                    ],
                ),
                TagEntry::new("minecraft:camels_spawnable_on", vec![37, 39, 38]),
                TagEntry::new("minecraft:campfires", vec![859, 860]),
                TagEntry::new(
                    "minecraft:can_glide_through",
                    vec![366, 880, 881, 878, 879, 1108, 1107],
                ),
                TagEntry::new(
                    "minecraft:candle_cakes",
                    vec![
                        961, 962, 963, 964, 965, 966, 967, 968, 969, 970, 971, 972, 973, 974, 975,
                        976, 977,
                    ],
                ),
                TagEntry::new(
                    "minecraft:candles",
                    vec![
                        944, 945, 946, 947, 948, 949, 950, 951, 952, 953, 954, 955, 956, 957, 958,
                        959, 960,
                    ],
                ),
                TagEntry::new(
                    "minecraft:cannot_replace_below_tree_trunk",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 11],
                ),
                TagEntry::new("minecraft:cannot_support_kelp", vec![671]),
                TagEntry::new("minecraft:cannot_support_seagrass", vec![671]),
                TagEntry::new("minecraft:cannot_support_snow_layer", vec![277, 556, 524]),
                TagEntry::new("minecraft:cauldrons", vec![387, 388, 389, 390]),
                TagEntry::new("minecraft:cave_vines", vec![1108, 1107]),
                TagEntry::new(
                    "minecraft:ceiling_hanging_signs",
                    vec![234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245],
                ),
                TagEntry::new(
                    "minecraft:chains",
                    vec![350, 351, 355, 352, 356, 353, 357, 354, 358],
                ),
                TagEntry::new("minecraft:cherry_logs", vec![54, 76, 65, 84]),
                TagEntry::new(
                    "minecraft:climbable",
                    vec![221, 366, 837, 878, 879, 880, 881, 1107, 1108],
                ),
                TagEntry::new("minecraft:coal_ores", vec![46, 47]),
                TagEntry::new(
                    "minecraft:combination_step_sound_blocks",
                    vec![
                        538, 539, 540, 541, 542, 543, 544, 545, 546, 547, 548, 549, 550, 551, 552,
                        553, 1112, 1161, 276, 870, 869, 882, 368,
                    ],
                ),
                TagEntry::new(
                    "minecraft:completes_find_tree_tutorial",
                    vec![
                        55, 77, 66, 85, 56, 20, 67, 86, 49, 71, 68, 79, 53, 75, 64, 83, 51, 73, 62,
                        81, 52, 74, 63, 82, 50, 72, 61, 80, 57, 78, 69, 87, 54, 76, 65, 84, 871,
                        872, 873, 874, 862, 863, 864, 865, 91, 88, 89, 95, 94, 92, 90, 97, 98, 96,
                        93, 672, 868,
                    ],
                ),
                TagEntry::new(
                    "minecraft:concrete_powder",
                    vec![
                        726, 727, 728, 729, 730, 731, 732, 733, 734, 735, 736, 737, 738, 739, 740,
                        741,
                    ],
                ),
                TagEntry::new("minecraft:convertable_to_mud", vec![9, 10, 1121]),
                TagEntry::new(
                    "minecraft:copper",
                    vec![1007, 1008, 1009, 1010, 1033, 1035, 1034, 1036],
                ),
                TagEntry::new(
                    "minecraft:copper_chests",
                    vec![1081, 1082, 1083, 1084, 1085, 1086, 1087, 1088],
                ),
                TagEntry::new(
                    "minecraft:copper_golem_statues",
                    vec![1089, 1090, 1091, 1092, 1093, 1094, 1095, 1096],
                ),
                TagEntry::new("minecraft:copper_ores", vec![1011, 1012]),
                TagEntry::new("minecraft:coral_blocks", vec![753, 754, 755, 756, 757]),
                TagEntry::new("minecraft:coral_plants", vec![763, 764, 765, 766, 767]),
                TagEntry::new(
                    "minecraft:corals",
                    vec![763, 764, 765, 766, 767, 773, 774, 775, 776, 777],
                ),
                TagEntry::new("minecraft:crimson_stems", vec![871, 872, 873, 874]),
                TagEntry::new(
                    "minecraft:crops",
                    vec![665, 441, 442, 207, 365, 364, 662, 663],
                ),
                TagEntry::new("minecraft:crystal_sound_blocks", vec![978, 979]),
                TagEntry::new(
                    "minecraft:dampens_vibrations",
                    vec![
                        140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154,
                        155, 538, 539, 540, 541, 542, 543, 544, 545, 546, 547, 548, 549, 550, 551,
                        552, 553,
                    ],
                ),
                TagEntry::new("minecraft:dark_oak_logs", vec![55, 77, 66, 85]),
                TagEntry::new("minecraft:deepslate_ore_replaceables", vec![1123, 984]),
                TagEntry::new("minecraft:diamond_ores", vec![203, 204]),
                TagEntry::new("minecraft:dirt", vec![9, 10, 1121]),
                TagEntry::new("minecraft:does_not_block_hoppers", vec![911, 912]),
                TagEntry::new(
                    "minecraft:doors",
                    vec![
                        220, 646, 647, 648, 649, 651, 652, 899, 900, 653, 654, 650, 1049, 1050,
                        1052, 1051, 1053, 1054, 1056, 1055, 260,
                    ],
                ),
                TagEntry::new(
                    "minecraft:dragon_immune",
                    vec![
                        524, 34, 391, 392, 667, 407, 668, 669, 905, 906, 156, 193, 917, 393, 341,
                        918, 1154, 907, 908,
                    ],
                ),
                TagEntry::new("minecraft:dragon_transparent", vec![525, 196, 197]),
                TagEntry::new(
                    "minecraft:dripstone_replaceable_blocks",
                    vec![1, 2, 4, 6, 984, 1123],
                ),
                TagEntry::new("minecraft:edible_for_sheep", vec![130, 134, 135, 131]),
                TagEntry::new("minecraft:emerald_ores", vec![398, 399]),
                TagEntry::new("minecraft:enables_bubble_column_drag_down", vec![671]),
                TagEntry::new("minecraft:enables_bubble_column_push_up", vec![286]),
                TagEntry::new("minecraft:enchantment_power_provider", vec![178]),
                TagEntry::new(
                    "minecraft:enchantment_power_transmitter",
                    vec![
                        0, 35, 36, 130, 131, 132, 133, 134, 135, 136, 137, 196, 197, 276, 366, 367,
                        368, 525, 561, 562, 675, 794, 795, 796, 869, 870, 882, 1115, 1120,
                    ],
                ),
                TagEntry::new(
                    "minecraft:enderman_holdable",
                    vec![
                        157, 1163, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 170, 159,
                        1164, 158, 9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 37, 39, 40, 172,
                        173, 177, 279, 281, 360, 296, 361, 876, 875, 882, 867, 866, 869, 280,
                    ],
                ),
                TagEntry::new(
                    "minecraft:fall_damage_resetting",
                    vec![221, 366, 837, 878, 879, 880, 881, 1107, 1108, 861, 129],
                ),
                TagEntry::new(
                    "minecraft:features_cannot_replace",
                    vec![34, 198, 201, 392, 1154, 1157, 1158],
                ),
                TagEntry::new(
                    "minecraft:fence_gates",
                    vec![631, 629, 633, 634, 630, 369, 628, 893, 894, 635, 636, 632],
                ),
                TagEntry::new(
                    "minecraft:fences",
                    vec![
                        284, 640, 642, 643, 637, 638, 639, 889, 890, 644, 645, 641, 382,
                    ],
                ),
                TagEntry::new("minecraft:fire", vec![196, 197]),
                TagEntry::new(
                    "minecraft:flower_pots",
                    vec![
                        411, 1165, 1166, 425, 426, 427, 428, 429, 430, 431, 432, 433, 423, 413,
                        414, 415, 416, 417, 419, 420, 437, 438, 439, 422, 440, 434, 435, 436, 793,
                        919, 920, 921, 922, 1148, 1149, 421, 418, 412, 424,
                    ],
                ),
                TagEntry::new(
                    "minecraft:flowers",
                    vec![
                        157, 1163, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 170, 159,
                        1164, 158, 557, 558, 560, 559, 664, 98, 1111, 33, 93, 1113, 1114, 657,
                        1109, 280,
                    ],
                ),
                TagEntry::new(
                    "minecraft:forest_rock_can_place_on",
                    vec![
                        9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 1, 2, 4, 6, 984, 1123,
                    ],
                ),
                TagEntry::new("minecraft:foxes_spawnable_on", vec![8, 276, 278, 11, 10]),
                TagEntry::new("minecraft:frog_prefer_jump_to", vec![374, 1117]),
                TagEntry::new("minecraft:frogs_spawnable_on", vec![8, 1122, 58, 59]),
                TagEntry::new(
                    "minecraft:geode_invalid_blocks",
                    vec![34, 35, 36, 277, 556, 789],
                ),
                TagEntry::new(
                    "minecraft:goats_spawnable_on",
                    vec![8, 1, 276, 278, 556, 40],
                ),
                TagEntry::new("minecraft:gold_ores", vec![42, 48, 43]),
                TagEntry::new("minecraft:grass_blocks", vec![8, 11, 373]),
                TagEntry::new("minecraft:grows_crops", vec![208]),
                TagEntry::new(
                    "minecraft:guarded_by_piglins",
                    vec![
                        1081, 1082, 1083, 1084, 1085, 1086, 1087, 1088, 174, 839, 201, 400, 935,
                        470, 1147, 677, 693, 689, 690, 687, 685, 691, 681, 686, 683, 680, 679, 684,
                        688, 692, 678, 682, 42, 48, 43,
                    ],
                ),
                TagEntry::new(
                    "minecraft:happy_ghast_avoids",
                    vec![861, 279, 170, 671, 196, 1105],
                ),
                TagEntry::new("minecraft:hoglin_repellents", vec![867, 920, 295, 918]),
                TagEntry::new(
                    "minecraft:huge_brown_mushroom_can_place_on",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 875, 866],
                ),
                TagEntry::new(
                    "minecraft:huge_red_mushroom_can_place_on",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 875, 866],
                ),
                TagEntry::new("minecraft:ice", vec![277, 556, 789, 670]),
                TagEntry::new(
                    "minecraft:ice_spike_replaceable",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 278, 277],
                ),
                TagEntry::new(
                    "minecraft:impermeable",
                    vec![
                        101, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313,
                        314, 315, 999, 524,
                    ],
                ),
                TagEntry::new(
                    "minecraft:incorrect_for_copper_tool",
                    vec![
                        193, 917, 915, 918, 916, 205, 203, 204, 398, 399, 403, 174, 1147, 42, 43,
                        271, 272,
                    ],
                ),
                TagEntry::new("minecraft:incorrect_for_diamond_tool", vec![]),
                TagEntry::new(
                    "minecraft:incorrect_for_gold_tool",
                    vec![
                        193, 917, 915, 918, 916, 205, 203, 204, 398, 399, 403, 174, 1147, 42, 43,
                        271, 272, 175, 1145, 44, 45, 104, 102, 103, 1007, 1146, 1011, 1012, 1032,
                        1028, 1016, 1009, 1030, 1026, 1014, 1010, 1029, 1025, 1013, 1008, 1031,
                        1027, 1015, 1033, 1048, 1044, 1040, 1034, 1046, 1042, 1038, 1035, 1047,
                        1043, 1039, 1036, 1045, 1041, 1037, 1156, 1020, 1019, 1018, 1017, 1024,
                        1023, 1022, 1021, 1065, 1066, 1067, 1068, 1069, 1070, 1071, 1072, 1073,
                        1074, 1075, 1076, 1077, 1078, 1079, 1080, 1057, 1058, 1060, 1059, 1061,
                        1062, 1064, 1063, 1081, 1082, 1083, 1084, 1085, 1086, 1087, 1088, 1097,
                        1098, 1099, 1100, 1101, 1102, 1103, 1104,
                    ],
                ),
                TagEntry::new(
                    "minecraft:incorrect_for_iron_tool",
                    vec![193, 917, 915, 918, 916],
                ),
                TagEntry::new("minecraft:incorrect_for_netherite_tool", vec![]),
                TagEntry::new(
                    "minecraft:incorrect_for_stone_tool",
                    vec![
                        193, 917, 915, 918, 916, 205, 203, 204, 398, 399, 403, 174, 1147, 42, 43,
                        271, 272,
                    ],
                ),
                TagEntry::new(
                    "minecraft:incorrect_for_wooden_tool",
                    vec![
                        193, 917, 915, 918, 916, 205, 203, 204, 398, 399, 403, 174, 1147, 42, 43,
                        271, 272, 175, 1145, 44, 45, 104, 102, 103, 1007, 1146, 1011, 1012, 1032,
                        1028, 1016, 1009, 1030, 1026, 1014, 1010, 1029, 1025, 1013, 1008, 1031,
                        1027, 1015, 1033, 1048, 1044, 1040, 1034, 1046, 1042, 1038, 1035, 1047,
                        1043, 1039, 1036, 1045, 1041, 1037, 1156, 1020, 1019, 1018, 1017, 1024,
                        1023, 1022, 1021, 1065, 1066, 1067, 1068, 1069, 1070, 1071, 1072, 1073,
                        1074, 1075, 1076, 1077, 1078, 1079, 1080, 1057, 1058, 1060, 1059, 1061,
                        1062, 1064, 1063, 1081, 1082, 1083, 1084, 1085, 1086, 1087, 1088, 1097,
                        1098, 1099, 1100, 1101, 1102, 1103, 1104,
                    ],
                ),
                TagEntry::new("minecraft:infiniburn_end", vec![285, 671, 34]),
                TagEntry::new("minecraft:infiniburn_nether", vec![285, 671]),
                TagEntry::new("minecraft:infiniburn_overworld", vec![285, 671]),
                TagEntry::new(
                    "minecraft:inside_step_sound_blocks",
                    vec![1000, 1004, 367, 374, 983, 1113, 1114, 1115],
                ),
                TagEntry::new("minecraft:invalid_spawn_inside", vec![391, 667]),
                TagEntry::new("minecraft:iron_ores", vec![44, 45]),
                TagEntry::new("minecraft:jungle_logs", vec![52, 74, 63, 82]),
                TagEntry::new(
                    "minecraft:lanterns",
                    vec![849, 850, 851, 855, 852, 856, 853, 857, 854, 858],
                ),
                TagEntry::new("minecraft:lapis_ores", vec![102, 103]),
                TagEntry::new(
                    "minecraft:lava_pool_stone_cannot_replace",
                    vec![
                        34, 198, 201, 392, 1154, 1157, 1158, 91, 88, 89, 95, 94, 92, 90, 97, 98,
                        96, 93, 55, 77, 66, 85, 56, 20, 67, 86, 49, 71, 68, 79, 53, 75, 64, 83, 51,
                        73, 62, 81, 52, 74, 63, 82, 50, 72, 61, 80, 57, 78, 69, 87, 54, 76, 65, 84,
                        871, 872, 873, 874, 862, 863, 864, 865,
                    ],
                ),
                TagEntry::new(
                    "minecraft:leaves",
                    vec![91, 88, 89, 95, 94, 92, 90, 97, 98, 96, 93],
                ),
                TagEntry::new(
                    "minecraft:lightning_rods",
                    vec![1097, 1098, 1099, 1100, 1101, 1102, 1103, 1104],
                ),
                TagEntry::new(
                    "minecraft:logs",
                    vec![
                        55, 77, 66, 85, 56, 20, 67, 86, 49, 71, 68, 79, 53, 75, 64, 83, 51, 73, 62,
                        81, 52, 74, 63, 82, 50, 72, 61, 80, 57, 78, 69, 87, 54, 76, 65, 84, 871,
                        872, 873, 874, 862, 863, 864, 865,
                    ],
                ),
                TagEntry::new(
                    "minecraft:logs_that_burn",
                    vec![
                        55, 77, 66, 85, 56, 20, 67, 86, 49, 71, 68, 79, 53, 75, 64, 83, 51, 73, 62,
                        81, 52, 74, 63, 82, 50, 72, 61, 80, 57, 78, 69, 87, 54, 76, 65, 84,
                    ],
                ),
                TagEntry::new(
                    "minecraft:lush_ground_replaceable",
                    vec![
                        1, 2, 4, 6, 984, 1123, 1108, 1107, 9, 10, 1121, 1122, 59, 1116, 1160, 8,
                        11, 373, 281, 40, 37,
                    ],
                ),
                TagEntry::new(
                    "minecraft:maintains_farmland",
                    vec![
                        364, 362, 365, 363, 665, 441, 442, 662, 159, 663, 207, 156, 631, 629, 633,
                        634, 630, 369, 628, 893, 894, 635, 636, 632,
                    ],
                ),
                TagEntry::new("minecraft:mangrove_logs", vec![57, 78, 69, 87]),
                TagEntry::new(
                    "minecraft:mangrove_logs_can_grow_through",
                    vec![1122, 59, 58, 96, 57, 33, 1112, 366],
                ),
                TagEntry::new(
                    "minecraft:mangrove_roots_can_grow_through",
                    vec![1122, 59, 58, 1112, 366, 33, 276],
                ),
                TagEntry::new(
                    "minecraft:mineable/axe",
                    vec![
                        109, 792, 839, 911, 912, 1118, 1117, 178, 338, 859, 842, 296, 201, 657,
                        656, 396, 909, 206, 474, 843, 367, 297, 283, 221, 845, 838, 361, 340, 360,
                        339, 846, 860, 470, 366, 563, 564, 565, 566, 567, 568, 569, 570, 571, 572,
                        573, 574, 575, 576, 577, 578, 579, 580, 581, 582, 583, 584, 585, 586, 587,
                        588, 589, 590, 591, 592, 593, 594, 631, 629, 633, 634, 630, 369, 628, 893,
                        894, 635, 636, 632, 55, 77, 66, 85, 56, 20, 67, 86, 49, 71, 68, 79, 53, 75,
                        64, 83, 51, 73, 62, 81, 52, 74, 63, 82, 50, 72, 61, 80, 57, 78, 69, 87, 54,
                        76, 65, 84, 871, 872, 873, 874, 862, 863, 864, 865, 13, 14, 15, 16, 17, 19,
                        21, 883, 884, 22, 23, 18, 210, 211, 212, 213, 215, 216, 217, 901, 902, 218,
                        219, 214, 224, 225, 226, 227, 229, 230, 231, 903, 904, 232, 233, 228, 443,
                        444, 445, 446, 447, 449, 450, 897, 898, 451, 452, 448, 220, 646, 647, 648,
                        649, 651, 652, 899, 900, 653, 654, 650, 284, 640, 642, 643, 637, 638, 639,
                        889, 890, 644, 645, 641, 261, 262, 263, 264, 265, 267, 268, 887, 888, 269,
                        270, 266, 599, 600, 601, 602, 603, 605, 606, 885, 886, 607, 608, 604, 200,
                        404, 405, 406, 516, 518, 519, 895, 896, 520, 521, 517, 320, 318, 322, 323,
                        319, 316, 317, 891, 892, 324, 325, 321, 58, 234, 235, 236, 237, 238, 239,
                        240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 255,
                        256, 254, 257, 24, 609, 522, 60, 70, 179, 180, 181, 182, 183, 184, 185,
                        186, 187, 188, 189, 190, 191, 199,
                    ],
                ),
                TagEntry::new(
                    "minecraft:mineable/hoe",
                    vec![
                        91, 88, 89, 95, 94, 92, 90, 97, 98, 96, 93, 672, 868, 537, 744, 910, 877,
                        99, 100, 1001, 1002, 1116, 1112, 1160, 1161, 1003, 1005, 1004, 1006,
                    ],
                ),
                TagEntry::new(
                    "minecraft:mineable/pickaxe",
                    vec![
                        1, 2, 3, 4, 5, 6, 7, 12, 42, 43, 44, 45, 46, 47, 48, 102, 103, 104, 105,
                        106, 107, 108, 174, 175, 176, 192, 193, 198, 203, 204, 205, 209, 223, 259,
                        260, 271, 272, 285, 288, 289, 326, 327, 328, 329, 370, 371, 381, 382, 383,
                        385, 386, 393, 397, 398, 399, 400, 403, 471, 472, 475, 476, 477, 478, 479,
                        480, 481, 483, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495,
                        496, 497, 498, 499, 526, 527, 528, 529, 530, 531, 532, 533, 534, 535, 554,
                        555, 595, 596, 597, 598, 610, 611, 612, 613, 614, 615, 616, 617, 619, 620,
                        621, 622, 623, 624, 625, 626, 627, 658, 659, 660, 661, 671, 673, 674, 676,
                        694, 695, 696, 697, 698, 699, 700, 701, 702, 703, 704, 705, 706, 707, 708,
                        709, 710, 711, 712, 713, 714, 715, 716, 717, 718, 719, 720, 721, 722, 723,
                        724, 725, 748, 749, 750, 751, 752, 753, 754, 755, 756, 757, 758, 759, 760,
                        761, 762, 768, 769, 770, 771, 772, 778, 779, 780, 781, 782, 797, 798, 799,
                        800, 801, 802, 803, 804, 805, 806, 807, 808, 809, 810, 811, 812, 813, 814,
                        815, 816, 817, 818, 819, 820, 821, 822, 823, 840, 841, 844, 847, 848, 866,
                        875, 915, 916, 917, 918, 923, 924, 925, 927, 928, 929, 930, 931, 932, 933,
                        935, 936, 937, 938, 941, 942, 943, 984, 998, 1010, 1009, 1008, 1007, 1011,
                        1012, 1013, 1014, 1015, 1016, 1025, 1026, 1027, 1028, 1029, 1030, 1031,
                        1032, 1033, 1034, 1035, 1036, 1037, 1038, 1039, 1040, 1041, 1042, 1043,
                        1044, 1045, 1046, 1047, 1048, 1105, 1106, 1123, 1124, 1125, 1126, 1128,
                        1129, 1130, 1132, 1133, 1134, 1136, 1137, 1138, 1140, 1141, 1142, 1144,
                        1145, 1146, 1147, 277, 556, 789, 138, 128, 139, 980, 983, 982, 981, 978,
                        979, 333, 337, 336, 1143, 332, 335, 334, 275, 939, 409, 410, 824, 825, 826,
                        827, 828, 829, 831, 832, 833, 834, 835, 836, 926, 934, 940, 1127, 1131,
                        1135, 1139, 830, 987, 991, 996, 379, 677, 693, 689, 690, 687, 685, 691,
                        681, 686, 683, 680, 679, 684, 688, 692, 678, 682, 467, 468, 469, 387, 388,
                        389, 390, 222, 126, 127, 482, 790, 331, 372, 618, 330, 1156, 985, 986, 992,
                        988, 989, 990, 993, 994, 995, 997, 1020, 1019, 1018, 1017, 1024, 1023,
                        1022, 1021, 1065, 1066, 1067, 1068, 1069, 1070, 1071, 1072, 1073, 1074,
                        1075, 1076, 1077, 1078, 1079, 1080, 1049, 1050, 1052, 1051, 1053, 1054,
                        1056, 1055, 1057, 1058, 1060, 1059, 1061, 1062, 1064, 1063, 1159, 376, 378,
                        377, 380, 1081, 1082, 1083, 1084, 1085, 1086, 1087, 1088, 1089, 1090, 1091,
                        1092, 1093, 1094, 1095, 1096, 1097, 1098, 1099, 1100, 1101, 1102, 1103,
                        1104, 849, 850, 851, 855, 852, 856, 853, 857, 854, 858, 350, 351, 355, 352,
                        356, 353, 357, 354, 358, 341, 342, 346, 343, 347, 344, 348, 345, 349,
                    ],
                ),
                TagEntry::new(
                    "minecraft:mineable/shovel",
                    vec![
                        281, 9, 10, 11, 208, 8, 40, 373, 37, 39, 278, 276, 286, 666, 287, 1121, 59,
                        1122, 38, 41, 726, 727, 728, 729, 730, 731, 732, 733, 734, 735, 736, 737,
                        738, 739, 740, 741,
                    ],
                ),
                TagEntry::new(
                    "minecraft:mob_interactable_doors",
                    vec![
                        220, 646, 647, 648, 649, 651, 652, 899, 900, 653, 654, 650, 1049, 1050,
                        1052, 1051, 1053, 1054, 1056, 1055,
                    ],
                ),
                TagEntry::new("minecraft:mooshrooms_spawnable_on", vec![373]),
                TagEntry::new("minecraft:moss_blocks", vec![1116, 1160]),
                TagEntry::new(
                    "minecraft:moss_replaceable",
                    vec![
                        1, 2, 4, 6, 984, 1123, 1108, 1107, 9, 10, 1121, 1122, 59, 1116, 1160, 8,
                        11, 373,
                    ],
                ),
                TagEntry::new("minecraft:mud", vec![1122, 59]),
                TagEntry::new(
                    "minecraft:needs_diamond_tool",
                    vec![193, 917, 915, 918, 916],
                ),
                TagEntry::new(
                    "minecraft:needs_iron_tool",
                    vec![205, 203, 204, 398, 399, 403, 174, 1147, 42, 43, 271, 272],
                ),
                TagEntry::new(
                    "minecraft:needs_stone_tool",
                    vec![
                        175, 1145, 44, 45, 104, 102, 103, 1007, 1146, 1011, 1012, 1032, 1028, 1016,
                        1009, 1030, 1026, 1014, 1010, 1029, 1025, 1013, 1008, 1031, 1027, 1015,
                        1033, 1048, 1044, 1040, 1034, 1046, 1042, 1038, 1035, 1047, 1043, 1039,
                        1036, 1045, 1041, 1037, 1156, 1020, 1019, 1018, 1017, 1024, 1023, 1022,
                        1021, 1065, 1066, 1067, 1068, 1069, 1070, 1071, 1072, 1073, 1074, 1075,
                        1076, 1077, 1078, 1079, 1080, 1057, 1058, 1060, 1059, 1061, 1062, 1064,
                        1063, 1081, 1082, 1083, 1084, 1085, 1086, 1087, 1088, 1097, 1098, 1099,
                        1100, 1101, 1102, 1103, 1104,
                    ],
                ),
                TagEntry::new(
                    "minecraft:nether_carver_replaceables",
                    vec![
                        1, 2, 4, 6, 984, 1123, 285, 288, 924, 9, 10, 1121, 1122, 59, 1116, 1160, 8,
                        11, 373, 875, 866, 672, 868, 286, 287,
                    ],
                ),
                TagEntry::new("minecraft:nylium", vec![875, 866]),
                TagEntry::new("minecraft:oak_logs", vec![49, 71, 68, 79]),
                TagEntry::new(
                    "minecraft:occludes_vibration_signals",
                    vec![
                        140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154,
                        155,
                    ],
                ),
                TagEntry::new(
                    "minecraft:overrides_mushroom_light_requirement",
                    vec![373, 11, 875, 866],
                ),
                TagEntry::new(
                    "minecraft:overworld_carver_replaceables",
                    vec![
                        1, 2, 4, 6, 984, 1123, 9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 37,
                        39, 38, 554, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495,
                        496, 497, 498, 499, 44, 45, 1011, 1012, 276, 278, 1000, 35, 40, 41, 106,
                        595, 998, 556, 1145, 1146,
                    ],
                ),
                TagEntry::new(
                    "minecraft:overworld_natural_logs",
                    vec![53, 51, 49, 52, 50, 55, 56, 57, 54],
                ),
                TagEntry::new("minecraft:pale_oak_logs", vec![56, 20, 67, 86]),
                TagEntry::new(
                    "minecraft:parrots_spawnable_on",
                    vec![
                        8, 0, 91, 88, 89, 95, 94, 92, 90, 97, 98, 96, 93, 55, 77, 66, 85, 56, 20,
                        67, 86, 49, 71, 68, 79, 53, 75, 64, 83, 51, 73, 62, 81, 52, 74, 63, 82, 50,
                        72, 61, 80, 57, 78, 69, 87, 54, 76, 65, 84, 871, 872, 873, 874, 862, 863,
                        864, 865,
                    ],
                ),
                TagEntry::new("minecraft:piglin_repellents", vec![197, 290, 850, 291, 860]),
                TagEntry::new(
                    "minecraft:planks",
                    vec![13, 14, 15, 16, 17, 19, 21, 883, 884, 22, 23, 18],
                ),
                TagEntry::new("minecraft:polar_bears_spawnable_on_alternate", vec![277]),
                TagEntry::new("minecraft:portals", vec![295, 391, 667]),
                TagEntry::new(
                    "minecraft:pressure_plates",
                    vec![
                        471, 472, 261, 262, 263, 264, 265, 267, 268, 887, 888, 269, 270, 266, 259,
                        938,
                    ],
                ),
                TagEntry::new(
                    "minecraft:prevent_mob_spawning_inside",
                    vec![222, 126, 127, 482],
                ),
                TagEntry::new(
                    "minecraft:prevents_nearby_leaf_decay",
                    vec![
                        55, 77, 66, 85, 56, 20, 67, 86, 49, 71, 68, 79, 53, 75, 64, 83, 51, 73, 62,
                        81, 52, 74, 63, 82, 50, 72, 61, 80, 57, 78, 69, 87, 54, 76, 65, 84, 871,
                        872, 873, 874, 862, 863, 864, 865,
                    ],
                ),
                TagEntry::new("minecraft:rabbits_spawnable_on", vec![8, 276, 278, 37]),
                TagEntry::new("minecraft:rails", vec![222, 126, 127, 482]),
                TagEntry::new("minecraft:redstone_ores", vec![271, 272]),
                TagEntry::new(
                    "minecraft:replaceable",
                    vec![
                        0, 35, 36, 130, 131, 132, 133, 134, 135, 136, 137, 196, 197, 276, 366, 367,
                        368, 525, 561, 562, 675, 794, 795, 796, 869, 870, 882, 1115, 1120,
                    ],
                ),
                TagEntry::new(
                    "minecraft:replaceable_by_mushrooms",
                    vec![
                        91, 88, 89, 95, 94, 92, 90, 97, 98, 96, 93, 157, 1163, 160, 161, 162, 163,
                        164, 165, 166, 167, 168, 169, 171, 170, 159, 1164, 158, 1161, 130, 131,
                        132, 366, 367, 557, 558, 559, 560, 561, 562, 1120, 664, 35, 136, 137, 172,
                        173, 338, 339, 869, 870, 882, 1115, 134, 135, 133, 1167,
                    ],
                ),
                TagEntry::new(
                    "minecraft:replaceable_by_trees",
                    vec![
                        91, 88, 89, 95, 94, 92, 90, 97, 98, 96, 93, 157, 1163, 160, 161, 162, 163,
                        164, 165, 166, 167, 168, 169, 171, 170, 159, 1164, 158, 1161, 130, 131,
                        132, 366, 367, 557, 558, 559, 560, 561, 562, 1120, 664, 35, 136, 137, 133,
                        1167, 869, 870, 882, 1115, 134, 135,
                    ],
                ),
                TagEntry::new("minecraft:sand", vec![37, 39, 38]),
                TagEntry::new(
                    "minecraft:saplings",
                    vec![25, 26, 27, 28, 29, 31, 32, 1110, 1111, 33, 30],
                ),
                TagEntry::new(
                    "minecraft:sculk_replaceable",
                    vec![
                        1, 2, 4, 6, 984, 1123, 9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 554,
                        484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498,
                        499, 875, 866, 285, 288, 924, 37, 39, 40, 286, 287, 998, 1144, 281, 1106,
                        393, 595, 106,
                    ],
                ),
                TagEntry::new(
                    "minecraft:sculk_replaceable_world_gen",
                    vec![
                        1, 2, 4, 6, 984, 1123, 9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 554,
                        484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498,
                        499, 875, 866, 285, 288, 924, 37, 39, 40, 286, 287, 998, 1144, 281, 1106,
                        393, 595, 106, 1136, 1132, 1124, 1141, 1142, 1128,
                    ],
                ),
                TagEntry::new(
                    "minecraft:shulker_boxes",
                    vec![
                        677, 693, 689, 690, 687, 685, 691, 681, 686, 683, 680, 679, 684, 688, 692,
                        678, 682,
                    ],
                ),
                TagEntry::new(
                    "minecraft:signs",
                    vec![
                        210, 211, 212, 213, 215, 216, 217, 901, 902, 218, 219, 214, 224, 225, 226,
                        227, 229, 230, 231, 903, 904, 232, 233, 228,
                    ],
                ),
                TagEntry::new(
                    "minecraft:slabs",
                    vec![
                        599, 600, 601, 602, 603, 605, 606, 885, 886, 607, 608, 604, 609, 610, 611,
                        617, 612, 623, 620, 621, 616, 615, 619, 614, 533, 534, 535, 811, 812, 813,
                        814, 815, 816, 817, 818, 819, 820, 821, 822, 823, 613, 622, 927, 932, 937,
                        1126, 1130, 1134, 1138, 1046, 1047, 1048, 1029, 1030, 1031, 1032, 1045,
                        618, 985, 989, 994, 378,
                    ],
                ),
                TagEntry::new(
                    "minecraft:small_flowers",
                    vec![
                        157, 1163, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 170, 159,
                        1164, 158,
                    ],
                ),
                TagEntry::new("minecraft:smelts_to_glass", vec![37, 39]),
                TagEntry::new(
                    "minecraft:snaps_goat_horn",
                    vec![
                        53, 51, 49, 52, 50, 55, 56, 57, 54, 1, 556, 44, 46, 1011, 398,
                    ],
                ),
                TagEntry::new(
                    "minecraft:sniffer_diggable_block",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11],
                ),
                TagEntry::new("minecraft:sniffer_egg_hatch_boost", vec![1116]),
                TagEntry::new("minecraft:snow", vec![276, 278, 1000]),
                TagEntry::new("minecraft:soul_fire_base_blocks", vec![286, 287]),
                TagEntry::new("minecraft:soul_speed_blocks", vec![286, 287]),
                TagEntry::new("minecraft:spruce_logs", vec![50, 72, 61, 80]),
                TagEntry::new(
                    "minecraft:stairs",
                    vec![
                        200, 404, 405, 406, 516, 518, 519, 895, 896, 520, 521, 517, 522, 223, 397,
                        383, 371, 370, 660, 481, 598, 531, 530, 532, 797, 798, 799, 800, 801, 802,
                        803, 804, 805, 806, 807, 808, 809, 810, 925, 933, 936, 1125, 1129, 1133,
                        1137, 1025, 1026, 1027, 1028, 1042, 1043, 1044, 1041, 372, 986, 990, 995,
                        377,
                    ],
                ),
                TagEntry::new(
                    "minecraft:standing_signs",
                    vec![210, 211, 212, 213, 215, 216, 217, 901, 902, 218, 219, 214],
                ),
                TagEntry::new("minecraft:stone_bricks", vec![326, 327, 328, 329]),
                TagEntry::new("minecraft:stone_buttons", vec![275, 939]),
                TagEntry::new("minecraft:stone_ore_replaceables", vec![1, 2, 4, 6]),
                TagEntry::new("minecraft:stone_pressure_plates", vec![259, 938]),
                TagEntry::new("minecraft:strider_warm_blocks", vec![36]),
                TagEntry::new(
                    "minecraft:substrate_overworld",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373],
                ),
                TagEntry::new("minecraft:support_override_cactus_flower", vec![279, 208]),
                TagEntry::new(
                    "minecraft:support_override_snow_layer",
                    vec![913, 286, 1122],
                ),
                TagEntry::new(
                    "minecraft:supports_azalea",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208, 281],
                ),
                TagEntry::new(
                    "minecraft:supports_bamboo",
                    vec![
                        37, 39, 38, 9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 792, 791, 40, 41,
                    ],
                ),
                TagEntry::new(
                    "minecraft:supports_big_dripleaf",
                    vec![281, 1116, 9, 8, 11, 10, 373, 1121, 1122, 59, 208],
                ),
                TagEntry::new("minecraft:supports_cactus", vec![37, 39, 38]),
                TagEntry::new("minecraft:supports_chorus_flower", vec![393]),
                TagEntry::new("minecraft:supports_chorus_plant", vec![393]),
                TagEntry::new("minecraft:supports_cocoa", vec![52, 74, 63, 82]),
                TagEntry::new(
                    "minecraft:supports_crimson_fungus",
                    vec![
                        9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208, 875, 866, 287,
                    ],
                ),
                TagEntry::new(
                    "minecraft:supports_crimson_roots",
                    vec![
                        9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208, 875, 866, 287,
                    ],
                ),
                TagEntry::new("minecraft:supports_crops", vec![208]),
                TagEntry::new(
                    "minecraft:supports_dry_vegetation",
                    vec![
                        37, 39, 38, 554, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494,
                        495, 496, 497, 498, 499, 9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373,
                        208,
                    ],
                ),
                TagEntry::new("minecraft:supports_frogspawn", vec![]),
                TagEntry::new("minecraft:supports_hanging_mangrove_propagule", vec![96]),
                TagEntry::new("minecraft:supports_lily_pad", vec![277, 670]),
                TagEntry::new(
                    "minecraft:supports_mangrove_propagule",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208, 281],
                ),
                TagEntry::new("minecraft:supports_melon_stem", vec![208]),
                TagEntry::new(
                    "minecraft:supports_melon_stem_fruit",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208],
                ),
                TagEntry::new(
                    "minecraft:supports_nether_sprouts",
                    vec![
                        9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208, 875, 866, 287,
                    ],
                ),
                TagEntry::new("minecraft:supports_nether_wart", vec![286]),
                TagEntry::new("minecraft:supports_pumpkin_stem", vec![208]),
                TagEntry::new(
                    "minecraft:supports_pumpkin_stem_fruit",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208],
                ),
                TagEntry::new("minecraft:supports_small_dripleaf", vec![281, 1116]),
                TagEntry::new("minecraft:supports_stem_crops", vec![208]),
                TagEntry::new(
                    "minecraft:supports_stem_fruit",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208],
                ),
                TagEntry::new(
                    "minecraft:supports_sugar_cane",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 37, 39, 38],
                ),
                TagEntry::new("minecraft:supports_sugar_cane_adjacently", vec![670]),
                TagEntry::new(
                    "minecraft:supports_vegetation",
                    vec![9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208],
                ),
                TagEntry::new(
                    "minecraft:supports_warped_fungus",
                    vec![
                        9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208, 875, 866, 287,
                    ],
                ),
                TagEntry::new(
                    "minecraft:supports_warped_roots",
                    vec![
                        9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208, 875, 866, 287,
                    ],
                ),
                TagEntry::new(
                    "minecraft:supports_wither_rose",
                    vec![
                        9, 10, 1121, 1122, 59, 1116, 1160, 8, 11, 373, 208, 285, 286, 287,
                    ],
                ),
                TagEntry::new(
                    "minecraft:sword_efficient",
                    vec![
                        91, 88, 89, 95, 94, 92, 90, 97, 98, 96, 93, 366, 367, 360, 296, 297, 361,
                        396, 1117, 1118, 656, 657,
                    ],
                ),
                TagEntry::new("minecraft:sword_instantly_mines", vec![792, 791]),
                TagEntry::new(
                    "minecraft:terracotta",
                    vec![
                        554, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497,
                        498, 499,
                    ],
                ),
                TagEntry::new("minecraft:trail_ruins_replaceable", vec![40]),
                TagEntry::new(
                    "minecraft:trapdoors",
                    vec![
                        320, 318, 322, 323, 319, 316, 317, 891, 892, 324, 325, 321, 526, 1057,
                        1058, 1060, 1059, 1061, 1062, 1064, 1063,
                    ],
                ),
                TagEntry::new(
                    "minecraft:triggers_ambient_desert_dry_vegetation_block_sounds",
                    vec![
                        554, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497,
                        498, 499, 37, 39,
                    ],
                ),
                TagEntry::new(
                    "minecraft:triggers_ambient_desert_sand_block_sounds",
                    vec![37, 39],
                ),
                TagEntry::new(
                    "minecraft:triggers_ambient_dried_ghast_block_sounds",
                    vec![286, 287],
                ),
                TagEntry::new(
                    "minecraft:underwater_bonemeals",
                    vec![
                        136, 763, 764, 765, 766, 767, 773, 774, 775, 776, 777, 783, 784, 785, 786,
                        787,
                    ],
                ),
                TagEntry::new(
                    "minecraft:unstable_bottom_center",
                    vec![631, 629, 633, 634, 630, 369, 628, 893, 894, 635, 636, 632],
                ),
                TagEntry::new("minecraft:valid_spawn", vec![8, 11]),
                TagEntry::new("minecraft:vibration_resonators", vec![978]),
                TagEntry::new("minecraft:wall_corals", vec![783, 784, 785, 786, 787]),
                TagEntry::new(
                    "minecraft:wall_hanging_signs",
                    vec![246, 247, 248, 249, 250, 251, 252, 253, 255, 256, 254, 257],
                ),
                TagEntry::new(
                    "minecraft:wall_post_override",
                    vec![
                        194, 290, 273, 292, 402, 210, 211, 212, 213, 215, 216, 217, 901, 902, 218,
                        219, 214, 224, 225, 226, 227, 229, 230, 231, 903, 904, 232, 233, 228, 563,
                        564, 565, 566, 567, 568, 569, 570, 571, 572, 573, 574, 575, 576, 577, 578,
                        579, 580, 581, 582, 583, 584, 585, 586, 587, 588, 589, 590, 591, 592, 593,
                        594, 471, 472, 261, 262, 263, 264, 265, 267, 268, 887, 888, 269, 270, 266,
                        259, 938, 280,
                    ],
                ),
                TagEntry::new(
                    "minecraft:wall_signs",
                    vec![224, 225, 226, 227, 229, 230, 231, 903, 904, 232, 233, 228],
                ),
                TagEntry::new(
                    "minecraft:walls",
                    vec![
                        409, 410, 824, 825, 826, 827, 828, 829, 831, 832, 833, 834, 835, 836, 926,
                        934, 940, 1127, 1131, 1135, 1139, 830, 987, 991, 996, 379,
                    ],
                ),
                TagEntry::new("minecraft:warped_stems", vec![862, 863, 864, 865]),
                TagEntry::new("minecraft:wart_blocks", vec![672, 868]),
                TagEntry::new(
                    "minecraft:wither_immune",
                    vec![
                        524, 34, 391, 392, 667, 407, 668, 669, 905, 906, 156, 525, 1154, 907, 908,
                    ],
                ),
                TagEntry::new("minecraft:wither_summon_base_blocks", vec![286, 287]),
                TagEntry::new("minecraft:wolves_spawnable_on", vec![8, 276, 278, 10, 11]),
                TagEntry::new(
                    "minecraft:wooden_buttons",
                    vec![443, 444, 445, 446, 447, 449, 450, 897, 898, 451, 452, 448],
                ),
                TagEntry::new(
                    "minecraft:wooden_doors",
                    vec![220, 646, 647, 648, 649, 651, 652, 899, 900, 653, 654, 650],
                ),
                TagEntry::new(
                    "minecraft:wooden_fences",
                    vec![284, 640, 642, 643, 637, 638, 639, 889, 890, 644, 645, 641],
                ),
                TagEntry::new(
                    "minecraft:wooden_pressure_plates",
                    vec![261, 262, 263, 264, 265, 267, 268, 887, 888, 269, 270, 266],
                ),
                TagEntry::new(
                    "minecraft:wooden_shelves",
                    vec![180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191],
                ),
                TagEntry::new(
                    "minecraft:wooden_slabs",
                    vec![599, 600, 601, 602, 603, 605, 606, 885, 886, 607, 608, 604],
                ),
                TagEntry::new(
                    "minecraft:wooden_stairs",
                    vec![200, 404, 405, 406, 516, 518, 519, 895, 896, 520, 521, 517],
                ),
                TagEntry::new(
                    "minecraft:wooden_trapdoors",
                    vec![320, 318, 322, 323, 319, 316, 317, 891, 892, 324, 325, 321],
                ),
                TagEntry::new(
                    "minecraft:wool",
                    vec![
                        140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154,
                        155,
                    ],
                ),
                TagEntry::new(
                    "minecraft:wool_carpets",
                    vec![
                        538, 539, 540, 541, 542, 543, 544, 545, 546, 547, 548, 549, 550, 551, 552,
                        553,
                    ],
                ),
            ],
        ),
        TagRegistry::new(
            "minecraft:damage_type",
            vec![
                TagEntry::new("minecraft:always_hurts_ender_dragons", vec![15, 9, 35, 1]),
                TagEntry::new(
                    "minecraft:always_kills_armor_stands",
                    vec![0, 45, 14, 49, 47],
                ),
                TagEntry::new("minecraft:always_most_significant_fall", vec![32]),
                TagEntry::new("minecraft:always_triggers_silverfish", vec![27]),
                TagEntry::new(
                    "minecraft:avoids_guardian_thorns",
                    vec![27, 43, 15, 9, 35, 1],
                ),
                TagEntry::new("minecraft:burn_from_stepping", vec![3, 20]),
                TagEntry::new("minecraft:burns_armor_stands", vec![31]),
                TagEntry::new(
                    "minecraft:bypasses_armor",
                    vec![
                        31, 22, 4, 6, 16, 18, 48, 5, 40, 10, 8, 17, 39, 27, 23, 32, 19, 36, 33,
                    ],
                ),
                TagEntry::new("minecraft:bypasses_effects", vec![40]),
                TagEntry::new("minecraft:bypasses_enchantments", vec![36]),
                TagEntry::new("minecraft:bypasses_invulnerability", vec![32, 19]),
                TagEntry::new("minecraft:bypasses_resistance", vec![32, 19]),
                TagEntry::new(
                    "minecraft:bypasses_shield",
                    vec![
                        31, 22, 4, 6, 16, 18, 48, 5, 40, 10, 8, 17, 39, 27, 23, 32, 19, 36, 33, 2,
                        3, 7, 11, 13, 20, 21, 24, 25, 42,
                    ],
                ),
                TagEntry::new(
                    "minecraft:bypasses_wolf_armor",
                    vec![32, 19, 4, 6, 7, 17, 22, 23, 27, 33, 40, 43, 48],
                ),
                TagEntry::new("minecraft:can_break_armor_stand", vec![35, 34, 37, 26]),
                TagEntry::new("minecraft:damages_helmet", vec![11, 12, 13]),
                TagEntry::new("minecraft:ignites_armor_stands", vec![21, 3]),
                TagEntry::new("minecraft:is_drowning", vec![6]),
                TagEntry::new("minecraft:is_explosion", vec![15, 9, 35, 1]),
                TagEntry::new("minecraft:is_fall", vec![10, 8, 39]),
                TagEntry::new("minecraft:is_fire", vec![21, 3, 31, 24, 20, 46, 14]),
                TagEntry::new("minecraft:is_freezing", vec![17]),
                TagEntry::new("minecraft:is_lightning", vec![25]),
                TagEntry::new("minecraft:is_player_attack", vec![34, 37, 26]),
                TagEntry::new(
                    "minecraft:is_projectile",
                    vec![0, 45, 30, 46, 14, 49, 44, 47],
                ),
                TagEntry::new("minecraft:mace_smash", vec![26]),
                TagEntry::new("minecraft:no_anger", vec![29]),
                TagEntry::new("minecraft:no_impact", vec![6]),
                TagEntry::new(
                    "minecraft:no_knockback",
                    vec![
                        9, 35, 1, 21, 25, 31, 24, 20, 22, 4, 6, 40, 2, 10, 8, 16, 32, 18, 27, 48,
                        5, 7, 42, 17, 39, 33, 19, 3, 37,
                    ],
                ),
                TagEntry::new(
                    "minecraft:panic_causes",
                    vec![
                        2, 17, 20, 21, 24, 25, 31, 0, 5, 9, 14, 15, 23, 27, 28, 30, 35, 36, 41, 44,
                        45, 46, 47, 48, 49, 34, 37, 26,
                    ],
                ),
                TagEntry::new(
                    "minecraft:panic_environmental_causes",
                    vec![2, 17, 20, 21, 24, 25, 31],
                ),
                TagEntry::new("minecraft:witch_resistant_to", vec![27, 23, 36, 43]),
                TagEntry::new("minecraft:wither_immune_to", vec![6]),
            ],
        ),
        TagRegistry::new(
            "minecraft:dialog",
            vec![
                TagEntry::new("minecraft:pause_screen_additions", vec![]),
                TagEntry::new("minecraft:quick_actions", vec![]),
            ],
        ),
        TagRegistry::new(
            "minecraft:enchantment",
            vec![
                TagEntry::new("minecraft:curse", vec![2, 41]),
                TagEntry::new(
                    "minecraft:double_trade_price",
                    vec![2, 41, 38, 36, 14, 23, 42],
                ),
                TagEntry::new("minecraft:exclusive_set/armor", vec![28, 3, 11, 27]),
                TagEntry::new("minecraft:exclusive_set/boots", vec![14, 7]),
                TagEntry::new("minecraft:exclusive_set/bow", vec![16, 23]),
                TagEntry::new("minecraft:exclusive_set/crossbow", vec![24, 25]),
                TagEntry::new("minecraft:exclusive_set/damage", vec![33, 35, 1, 15, 6, 4]),
                TagEntry::new("minecraft:exclusive_set/mining", vec![13, 34]),
                TagEntry::new("minecraft:exclusive_set/riptide", vec![19, 5]),
                TagEntry::new(
                    "minecraft:in_enchanting_table",
                    vec![
                        28, 11, 9, 3, 27, 31, 0, 39, 7, 33, 35, 1, 17, 10, 18, 37, 8, 34, 40, 13,
                        26, 29, 12, 16, 20, 22, 19, 15, 32, 5, 24, 30, 25, 6, 4, 21,
                    ],
                ),
                TagEntry::new(
                    "minecraft:non_treasure",
                    vec![
                        28, 11, 9, 3, 27, 31, 0, 39, 7, 33, 35, 1, 17, 10, 18, 37, 8, 34, 40, 13,
                        26, 29, 12, 16, 20, 22, 19, 15, 32, 5, 24, 30, 25, 6, 4, 21,
                    ],
                ),
                TagEntry::new(
                    "minecraft:on_mob_spawn_equipment",
                    vec![
                        28, 11, 9, 3, 27, 31, 0, 39, 7, 33, 35, 1, 17, 10, 18, 37, 8, 34, 40, 13,
                        26, 29, 12, 16, 20, 22, 19, 15, 32, 5, 24, 30, 25, 6, 4, 21,
                    ],
                ),
                TagEntry::new(
                    "minecraft:on_random_loot",
                    vec![
                        28, 11, 9, 3, 27, 31, 0, 39, 7, 33, 35, 1, 17, 10, 18, 37, 8, 34, 40, 13,
                        26, 29, 12, 16, 20, 22, 19, 15, 32, 5, 24, 30, 25, 6, 4, 21, 2, 41, 14, 23,
                    ],
                ),
                TagEntry::new(
                    "minecraft:on_traded_equipment",
                    vec![
                        28, 11, 9, 3, 27, 31, 0, 39, 7, 33, 35, 1, 17, 10, 18, 37, 8, 34, 40, 13,
                        26, 29, 12, 16, 20, 22, 19, 15, 32, 5, 24, 30, 25, 6, 4, 21,
                    ],
                ),
                TagEntry::new("minecraft:prevents_bee_spawns_when_mining", vec![34]),
                TagEntry::new("minecraft:prevents_decorated_pot_shattering", vec![34]),
                TagEntry::new("minecraft:prevents_ice_melting", vec![34]),
                TagEntry::new("minecraft:prevents_infested_spawns", vec![34]),
                TagEntry::new("minecraft:smelts_loot", vec![10]),
                TagEntry::new(
                    "minecraft:tooltip_order",
                    vec![
                        2, 41, 32, 5, 42, 14, 21, 33, 35, 1, 15, 26, 6, 4, 25, 37, 24, 10, 12, 17,
                        29, 28, 3, 11, 27, 9, 13, 18, 34, 20, 8, 30, 22, 31, 0, 36, 38, 7, 39, 19,
                        40, 16, 23,
                    ],
                ),
                TagEntry::new(
                    "minecraft:tradeable",
                    vec![
                        28, 11, 9, 3, 27, 31, 0, 39, 7, 33, 35, 1, 17, 10, 18, 37, 8, 34, 40, 13,
                        26, 29, 12, 16, 20, 22, 19, 15, 32, 5, 24, 30, 25, 6, 4, 21, 2, 41, 14, 23,
                    ],
                ),
                TagEntry::new("minecraft:treasure", vec![2, 41, 38, 36, 14, 23, 42]),
            ],
        ),
        TagRegistry::new(
            "minecraft:entity_type",
            vec![
                TagEntry::new("minecraft:accepts_iron_golem_gift", vec![28]),
                TagEntry::new(
                    "minecraft:aquatic",
                    vec![137, 7, 63, 40, 27, 107, 110, 136, 35, 127, 61, 130, 88, 152],
                ),
                TagEntry::new("minecraft:arrows", vec![6, 123]),
                TagEntry::new("minecraft:arthropod", vec![11, 42, 114, 124, 22]),
                TagEntry::new("minecraft:axolotl_always_hostiles", vec![38, 63, 40]),
                TagEntry::new(
                    "minecraft:axolotl_hunt_targets",
                    vec![136, 107, 110, 27, 127, 61, 130],
                ),
                TagEntry::new("minecraft:beehive_inhabitors", vec![11]),
                TagEntry::new(
                    "minecraft:boat",
                    vec![89, 125, 12, 74, 0, 23, 33, 94, 81, 9],
                ),
                TagEntry::new(
                    "minecraft:burn_in_daylight",
                    vec![115, 128, 146, 16, 150, 151, 153, 38, 152, 99],
                ),
                TagEntry::new(
                    "minecraft:can_breathe_under_water",
                    vec![
                        115, 128, 146, 116, 16, 97, 151, 20, 150, 153, 154, 149, 38, 67, 152, 145,
                        99, 7, 55, 63, 40, 137, 61, 27, 107, 110, 127, 136, 130, 5, 28, 88,
                    ],
                ),
                TagEntry::new("minecraft:can_equip_harness", vec![58]),
                TagEntry::new(
                    "minecraft:can_equip_saddle",
                    vec![66, 116, 151, 36, 87, 100, 129, 19, 20, 88, 152],
                ),
                TagEntry::new(
                    "minecraft:can_float_while_ridden",
                    vec![66, 151, 87, 36, 19, 20],
                ),
                TagEntry::new("minecraft:can_turn_in_boats", vec![17]),
                TagEntry::new("minecraft:can_wear_horse_armor", vec![66, 151]),
                TagEntry::new("minecraft:can_wear_nautilus_armor", vec![88, 152]),
                TagEntry::new("minecraft:candidate_for_iron_golem_gift", vec![139, 28]),
                TagEntry::new("minecraft:cannot_be_age_locked", vec![151, 116, 139]),
                TagEntry::new(
                    "minecraft:cannot_be_pushed_onto_boats",
                    vec![155, 40, 27, 107, 110, 136, 35, 127, 61, 130, 31, 88, 152],
                ),
                TagEntry::new("minecraft:deflects_projectiles", vec![17]),
                TagEntry::new(
                    "minecraft:dismounts_underwater",
                    vec![19, 26, 36, 58, 66, 78, 87, 100, 109, 124, 129, 134, 151],
                ),
                TagEntry::new(
                    "minecraft:fall_damage_immune",
                    vec![
                        28, 70, 121, 112, 2, 10, 11, 14, 21, 26, 57, 58, 99, 80, 91, 98, 145, 17,
                    ],
                ),
                TagEntry::new(
                    "minecraft:followable_friendly_mobs",
                    vec![
                        4, 11, 19, 21, 26, 30, 36, 54, 62, 58, 66, 116, 78, 87, 91, 96, 98, 100,
                        104, 108, 111, 119, 129, 139, 148,
                    ],
                ),
                TagEntry::new("minecraft:freeze_hurts_extra_types", vec![129, 14, 80]),
                TagEntry::new(
                    "minecraft:freeze_immune_entity_types",
                    vec![128, 104, 121, 145],
                ),
                TagEntry::new("minecraft:frog_food", vec![117, 80]),
                TagEntry::new(
                    "minecraft:ignores_poison_and_regen",
                    vec![
                        115, 128, 146, 116, 16, 97, 151, 20, 150, 153, 154, 149, 38, 67, 152, 145,
                        99,
                    ],
                ),
                TagEntry::new("minecraft:illager", vec![46, 68, 103, 140]),
                TagEntry::new("minecraft:illager_friends", vec![46, 68, 103, 140]),
                TagEntry::new("minecraft:immune_to_infested", vec![114]),
                TagEntry::new("minecraft:immune_to_oozing", vec![117]),
                TagEntry::new(
                    "minecraft:impact_projectiles",
                    vec![6, 123, 53, 120, 52, 118, 39, 135, 37, 147, 143, 18],
                ),
                TagEntry::new(
                    "minecraft:inverted_healing_and_harm",
                    vec![
                        115, 128, 146, 116, 16, 97, 151, 20, 150, 153, 154, 149, 38, 67, 152, 145,
                        99,
                    ],
                ),
                TagEntry::new("minecraft:nautilus_hostiles", vec![107]),
                TagEntry::new(
                    "minecraft:no_anger_from_wind_charge",
                    vec![17, 115, 16, 128, 150, 67, 124, 22, 117],
                ),
                TagEntry::new("minecraft:non_controlling_rider", vec![117, 80]),
                TagEntry::new(
                    "minecraft:not_scary_for_pufferfish",
                    vec![137, 63, 40, 27, 107, 110, 136, 35, 127, 61, 130, 88, 152],
                ),
                TagEntry::new(
                    "minecraft:powder_snow_walkable_mobs",
                    vec![108, 42, 114, 54],
                ),
                TagEntry::new("minecraft:raiders", vec![46, 103, 109, 140, 68, 144]),
                TagEntry::new("minecraft:redirectable_projectile", vec![52, 143, 18]),
                TagEntry::new(
                    "minecraft:sensitive_to_bane_of_arthropods",
                    vec![11, 42, 114, 124, 22],
                ),
                TagEntry::new(
                    "minecraft:sensitive_to_impaling",
                    vec![137, 7, 63, 40, 27, 107, 110, 136, 35, 127, 61, 130, 88, 152],
                ),
                TagEntry::new(
                    "minecraft:sensitive_to_smite",
                    vec![
                        115, 128, 146, 116, 16, 97, 151, 20, 150, 153, 154, 149, 38, 67, 152, 145,
                        99,
                    ],
                ),
                TagEntry::new("minecraft:skeletons", vec![115, 128, 146, 116, 16, 97]),
                TagEntry::new(
                    "minecraft:undead",
                    vec![
                        115, 128, 146, 116, 16, 97, 151, 20, 150, 153, 154, 149, 38, 67, 152, 145,
                        99,
                    ],
                ),
                TagEntry::new(
                    "minecraft:wither_friends",
                    vec![
                        115, 128, 146, 116, 16, 97, 151, 20, 150, 153, 154, 149, 38, 67, 152, 145,
                        99,
                    ],
                ),
                TagEntry::new(
                    "minecraft:zombies",
                    vec![151, 20, 150, 153, 154, 149, 38, 67, 152],
                ),
            ],
        ),
        TagRegistry::new(
            "minecraft:fluid",
            vec![
                TagEntry::new("minecraft:bubble_column_can_occupy", vec![2]),
                TagEntry::new("minecraft:lava", vec![4, 3]),
                TagEntry::new("minecraft:supports_frogspawn", vec![2]),
                TagEntry::new("minecraft:supports_lily_pad", vec![2]),
                TagEntry::new("minecraft:supports_sugar_cane_adjacently", vec![2, 1]),
                TagEntry::new("minecraft:water", vec![2, 1]),
            ],
        ),
        TagRegistry::new(
            "minecraft:game_event",
            vec![
                TagEntry::new("minecraft:allay_can_listen", vec![33]),
                TagEntry::new(
                    "minecraft:ignore_vibrations_sneaking",
                    vec![26, 36, 41, 42, 29, 28],
                ),
                TagEntry::new("minecraft:shrieker_can_listen", vec![37]),
                TagEntry::new(
                    "minecraft:vibrations",
                    vec![
                        1, 2, 3, 5, 6, 7, 8, 0, 4, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
                        21, 22, 24, 25, 26, 27, 28, 32, 33, 34, 35, 36, 38, 40, 41, 42, 43, 44, 45,
                        46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 23,
                    ],
                ),
                TagEntry::new(
                    "minecraft:warden_can_listen",
                    vec![
                        1, 2, 3, 5, 6, 7, 8, 0, 4, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
                        21, 22, 24, 25, 26, 27, 28, 32, 33, 34, 35, 36, 38, 40, 41, 42, 43, 44, 45,
                        46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 39, 37,
                    ],
                ),
            ],
        ),
        TagRegistry::new(
            "minecraft:instrument",
            vec![
                TagEntry::new("minecraft:goat_horns", vec![4, 6, 5, 3, 0, 1, 7, 2]),
                TagEntry::new("minecraft:regular_goat_horns", vec![4, 6, 5, 3]),
                TagEntry::new("minecraft:screaming_goat_horns", vec![0, 1, 7, 2]),
            ],
        ),
        TagRegistry::new(
            "minecraft:item",
            vec![
                TagEntry::new("minecraft:acacia_logs", vec![138, 175, 152, 163]),
                TagEntry::new("minecraft:anvil", vec![479, 480, 481]),
                TagEntry::new("minecraft:armadillo_food", vec![1123]),
                TagEntry::new("minecraft:arrows", vec![896, 1294, 1293]),
                TagEntry::new("minecraft:axes", vec![940, 925, 930, 945, 915, 935, 920]),
                TagEntry::new("minecraft:axolotl_food", vec![1023]),
                TagEntry::new("minecraft:bamboo_blocks", vec![147, 170]),
                TagEntry::new(
                    "minecraft:banners",
                    vec![
                        1267, 1268, 1269, 1270, 1271, 1272, 1273, 1274, 1275, 1276, 1277, 1278,
                        1279, 1280, 1281, 1282,
                    ],
                ),
                TagEntry::new(
                    "minecraft:bars",
                    vec![391, 392, 396, 393, 397, 394, 398, 395, 399],
                ),
                TagEntry::new(
                    "minecraft:beacon_payment_items",
                    vec![910, 900, 899, 909, 905],
                ),
                TagEntry::new(
                    "minecraft:beds",
                    vec![
                        1101, 1102, 1098, 1099, 1096, 1094, 1100, 1090, 1095, 1092, 1089, 1088,
                        1093, 1097, 1087, 1091,
                    ],
                ),
                TagEntry::new(
                    "minecraft:bee_food",
                    vec![
                        229, 231, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245,
                        525, 526, 528, 527, 246, 192, 206, 57, 187, 259, 260, 326, 247, 342,
                    ],
                ),
                TagEntry::new("minecraft:birch_logs", vec![136, 173, 150, 161]),
                TagEntry::new(
                    "minecraft:boats",
                    vec![
                        864, 866, 868, 870, 872, 876, 878, 880, 882, 874, 865, 867, 869, 871, 873,
                        877, 879, 881, 883, 875,
                    ],
                ),
                TagEntry::new("minecraft:book_cloning_target", vec![1221]),
                TagEntry::new(
                    "minecraft:bookshelf_books",
                    vec![1030, 1222, 1245, 1221, 1308],
                ),
                TagEntry::new(
                    "minecraft:breaks_decorated_pots",
                    vec![
                        937, 922, 927, 942, 912, 932, 917, 940, 925, 930, 945, 915, 935, 920, 939,
                        924, 929, 944, 914, 934, 919, 938, 923, 928, 943, 913, 933, 918, 941, 926,
                        931, 946, 916, 936, 921, 1332, 1224,
                    ],
                ),
                TagEntry::new("minecraft:brewing_fuel", vec![1125]),
                TagEntry::new(
                    "minecraft:bundles",
                    vec![
                        1037, 1053, 1049, 1050, 1047, 1045, 1051, 1041, 1046, 1043, 1040, 1039,
                        1044, 1048, 1052, 1042, 1038,
                    ],
                ),
                TagEntry::new(
                    "minecraft:buttons",
                    vec![
                        752, 753, 754, 755, 756, 758, 759, 762, 763, 760, 761, 757, 750, 751,
                    ],
                ),
                TagEntry::new("minecraft:camel_food", vec![341]),
                TagEntry::new("minecraft:camel_husk_food", vec![1253]),
                TagEntry::new(
                    "minecraft:candles",
                    vec![
                        1399, 1400, 1401, 1402, 1403, 1404, 1405, 1406, 1407, 1408, 1409, 1410,
                        1411, 1412, 1413, 1414, 1415,
                    ],
                ),
                TagEntry::new(
                    "minecraft:cat_collar_dyes",
                    vec![
                        1067, 1068, 1069, 1070, 1071, 1072, 1073, 1074, 1075, 1076, 1077, 1078,
                        1079, 1080, 1081, 1082,
                    ],
                ),
                TagEntry::new("minecraft:cat_food", vec![1058, 1059]),
                TagEntry::new(
                    "minecraft:cauldron_can_remove_dye",
                    vec![955, 956, 957, 958, 1261, 891],
                ),
                TagEntry::new(
                    "minecraft:chains",
                    vec![400, 401, 405, 402, 406, 403, 407, 404, 408],
                ),
                TagEntry::new("minecraft:cherry_logs", vec![139, 176, 153, 164]),
                TagEntry::new(
                    "minecraft:chest_armor",
                    vec![956, 960, 964, 976, 968, 972, 980],
                ),
                TagEntry::new(
                    "minecraft:chest_boats",
                    vec![865, 867, 869, 871, 873, 877, 879, 881, 883, 875],
                ),
                TagEntry::new(
                    "minecraft:chicken_food",
                    vec![952, 1110, 1109, 1289, 1286, 1287],
                ),
                TagEntry::new(
                    "minecraft:cluster_max_harvestables",
                    vec![939, 929, 934, 944, 924, 914, 919],
                ),
                TagEntry::new("minecraft:coal_ores", vec![64, 65]),
                TagEntry::new("minecraft:coals", vec![897, 898]),
                TagEntry::new("minecraft:compasses", vec![1035, 1036]),
                TagEntry::new(
                    "minecraft:completes_find_tree_tutorial",
                    vec![
                        141, 178, 154, 165, 140, 177, 155, 166, 134, 171, 148, 159, 138, 175, 152,
                        163, 136, 173, 150, 161, 137, 174, 151, 162, 135, 172, 149, 160, 142, 179,
                        156, 167, 139, 176, 153, 164, 145, 157, 180, 168, 146, 158, 181, 169, 185,
                        182, 183, 189, 188, 186, 184, 191, 192, 190, 187, 577, 578,
                    ],
                ),
                TagEntry::new("minecraft:copper", vec![91, 95, 96, 97, 114, 115, 116, 117]),
                TagEntry::new(
                    "minecraft:copper_chests",
                    vec![1485, 1486, 1487, 1488, 1489, 1490, 1491, 1492],
                ),
                TagEntry::new(
                    "minecraft:copper_golem_statues",
                    vec![1493, 1494, 1495, 1496, 1497, 1498, 1499, 1500],
                ),
                TagEntry::new("minecraft:copper_ores", vec![68, 69]),
                TagEntry::new("minecraft:copper_tool_materials", vec![907]),
                TagEntry::new("minecraft:cow_food", vec![953]),
                TagEntry::new(
                    "minecraft:creeper_drop_music_discs",
                    vec![
                        1310, 1311, 1312, 1313, 1316, 1318, 1319, 1320, 1321, 1322, 1323, 1324,
                    ],
                ),
                TagEntry::new("minecraft:creeper_igniters", vec![892, 1219]),
                TagEntry::new("minecraft:crimson_stems", vec![145, 157, 180, 168]),
                TagEntry::new(
                    "minecraft:dampens_vibrations",
                    vec![
                        213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227,
                        228, 506, 507, 508, 509, 510, 511, 512, 513, 514, 515, 516, 517, 518, 519,
                        520, 521,
                    ],
                ),
                TagEntry::new("minecraft:dark_oak_logs", vec![141, 178, 154, 165]),
                TagEntry::new(
                    "minecraft:decorated_pot_ingredients",
                    vec![
                        1026, 1446, 1447, 1448, 1449, 1450, 1451, 1452, 1453, 1455, 1457, 1458,
                        1459, 1460, 1461, 1462, 1463, 1465, 1466, 1467, 1468, 1454, 1456, 1464,
                    ],
                ),
                TagEntry::new(
                    "minecraft:decorated_pot_sherds",
                    vec![
                        1446, 1447, 1448, 1449, 1450, 1451, 1452, 1453, 1455, 1457, 1458, 1459,
                        1460, 1461, 1462, 1463, 1465, 1466, 1467, 1468, 1454, 1456, 1464,
                    ],
                ),
                TagEntry::new("minecraft:diamond_ores", vec![78, 79]),
                TagEntry::new("minecraft:diamond_tool_materials", vec![899]),
                TagEntry::new("minecraft:dirt", vec![28, 29, 31]),
                TagEntry::new(
                    "minecraft:doors",
                    vec![
                        781, 782, 783, 784, 785, 787, 788, 791, 792, 789, 790, 786, 793, 794, 795,
                        796, 797, 798, 799, 800, 780,
                    ],
                ),
                TagEntry::new("minecraft:drowned_preferred_weapons", vec![1332]),
                TagEntry::new("minecraft:duplicates_allays", vec![903]),
                TagEntry::new(
                    "minecraft:dyes",
                    vec![
                        1067, 1068, 1069, 1070, 1071, 1072, 1073, 1074, 1075, 1076, 1077, 1078,
                        1079, 1080, 1081, 1082,
                    ],
                ),
                TagEntry::new("minecraft:eggs", vec![1032, 1033, 1034]),
                TagEntry::new("minecraft:emerald_ores", vec![74, 75]),
                TagEntry::new(
                    "minecraft:enchantable/armor",
                    vec![
                        958, 962, 966, 978, 970, 974, 982, 957, 961, 965, 977, 969, 973, 981, 956,
                        960, 964, 976, 968, 972, 980, 955, 959, 963, 975, 967, 971, 979, 888,
                    ],
                ),
                TagEntry::new("minecraft:enchantable/bow", vec![895]),
                TagEntry::new(
                    "minecraft:enchantable/chest_armor",
                    vec![956, 960, 964, 976, 968, 972, 980],
                ),
                TagEntry::new("minecraft:enchantable/crossbow", vec![1340]),
                TagEntry::new(
                    "minecraft:enchantable/durability",
                    vec![
                        958, 962, 966, 978, 970, 974, 982, 957, 961, 965, 977, 969, 973, 981, 956,
                        960, 964, 976, 968, 972, 980, 955, 959, 963, 975, 967, 971, 979, 888, 863,
                        1296, 937, 922, 927, 942, 912, 932, 917, 940, 925, 930, 945, 915, 935, 920,
                        939, 924, 929, 944, 914, 934, 919, 938, 923, 928, 943, 913, 933, 918, 941,
                        926, 931, 946, 916, 936, 921, 895, 1340, 1332, 892, 1106, 1426, 1054, 860,
                        861, 1224, 1302, 1298, 1301, 1303, 1297, 1300, 1299,
                    ],
                ),
                TagEntry::new(
                    "minecraft:enchantable/equippable",
                    vec![
                        958, 962, 966, 978, 970, 974, 982, 957, 961, 965, 977, 969, 973, 981, 956,
                        960, 964, 976, 968, 972, 980, 955, 959, 963, 975, 967, 971, 979, 888, 863,
                        1236, 1238, 1237, 1234, 1235, 1239, 1240, 358,
                    ],
                ),
                TagEntry::new(
                    "minecraft:enchantable/fire_aspect",
                    vec![
                        937, 922, 927, 942, 912, 932, 917, 1302, 1298, 1301, 1303, 1297, 1300,
                        1299, 1224,
                    ],
                ),
                TagEntry::new("minecraft:enchantable/fishing", vec![1054]),
                TagEntry::new(
                    "minecraft:enchantable/foot_armor",
                    vec![958, 962, 966, 978, 970, 974, 982],
                ),
                TagEntry::new(
                    "minecraft:enchantable/head_armor",
                    vec![955, 959, 963, 975, 967, 971, 979, 888],
                ),
                TagEntry::new(
                    "minecraft:enchantable/leg_armor",
                    vec![957, 961, 965, 977, 969, 973, 981],
                ),
                TagEntry::new(
                    "minecraft:enchantable/lunge",
                    vec![1302, 1298, 1301, 1303, 1297, 1300, 1299],
                ),
                TagEntry::new("minecraft:enchantable/mace", vec![1224]),
                TagEntry::new(
                    "minecraft:enchantable/melee_weapon",
                    vec![
                        937, 922, 927, 942, 912, 932, 917, 1302, 1298, 1301, 1303, 1297, 1300, 1299,
                    ],
                ),
                TagEntry::new(
                    "minecraft:enchantable/mining",
                    vec![
                        940, 925, 930, 945, 915, 935, 920, 939, 924, 929, 944, 914, 934, 919, 938,
                        923, 928, 943, 913, 933, 918, 941, 926, 931, 946, 916, 936, 921, 1106,
                    ],
                ),
                TagEntry::new(
                    "minecraft:enchantable/mining_loot",
                    vec![
                        940, 925, 930, 945, 915, 935, 920, 939, 924, 929, 944, 914, 934, 919, 938,
                        923, 928, 943, 913, 933, 918, 941, 926, 931, 946, 916, 936, 921,
                    ],
                ),
                TagEntry::new(
                    "minecraft:enchantable/sharp_weapon",
                    vec![
                        937, 922, 927, 942, 912, 932, 917, 1302, 1298, 1301, 1303, 1297, 1300,
                        1299, 940, 925, 930, 945, 915, 935, 920,
                    ],
                ),
                TagEntry::new(
                    "minecraft:enchantable/sweeping",
                    vec![937, 922, 927, 942, 912, 932, 917],
                ),
                TagEntry::new("minecraft:enchantable/trident", vec![1332]),
                TagEntry::new(
                    "minecraft:enchantable/vanishing",
                    vec![
                        958, 962, 966, 978, 970, 974, 982, 957, 961, 965, 977, 969, 973, 981, 956,
                        960, 964, 976, 968, 972, 980, 955, 959, 963, 975, 967, 971, 979, 888, 863,
                        1296, 937, 922, 927, 942, 912, 932, 917, 940, 925, 930, 945, 915, 935, 920,
                        939, 924, 929, 944, 914, 934, 919, 938, 923, 928, 943, 913, 933, 918, 941,
                        926, 931, 946, 916, 936, 921, 895, 1340, 1332, 892, 1106, 1426, 1054, 860,
                        861, 1224, 1302, 1298, 1301, 1303, 1297, 1300, 1299, 1035, 358, 1236, 1238,
                        1237, 1234, 1235, 1239, 1240,
                    ],
                ),
                TagEntry::new(
                    "minecraft:enchantable/weapon",
                    vec![
                        937, 922, 927, 942, 912, 932, 917, 1302, 1298, 1301, 1303, 1297, 1300,
                        1299, 940, 925, 930, 945, 915, 935, 920, 1224,
                    ],
                ),
                TagEntry::new(
                    "minecraft:fence_gates",
                    vec![826, 824, 828, 829, 825, 822, 823, 832, 833, 830, 831, 827],
                ),
                TagEntry::new(
                    "minecraft:fences",
                    vec![
                        345, 349, 351, 352, 346, 347, 348, 355, 356, 353, 354, 350, 428,
                    ],
                ),
                TagEntry::new("minecraft:fishes", vec![1058, 1062, 1059, 1063, 1061, 1060]),
                TagEntry::new(
                    "minecraft:flowers",
                    vec![
                        229, 231, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245,
                        232, 230, 525, 526, 528, 527, 246, 192, 206, 57, 187, 259, 260, 326, 247,
                        342,
                    ],
                ),
                TagEntry::new(
                    "minecraft:foot_armor",
                    vec![958, 962, 966, 978, 970, 974, 982],
                ),
                TagEntry::new("minecraft:fox_food", vec![1374, 1375]),
                TagEntry::new(
                    "minecraft:freeze_immune_wearables",
                    vec![958, 957, 956, 955, 1261],
                ),
                TagEntry::new("minecraft:frog_food", vec![1031]),
                TagEntry::new("minecraft:furnace_minecart_fuel", vec![897, 898]),
                TagEntry::new("minecraft:gaze_disguise_equipment", vec![358]),
                TagEntry::new("minecraft:goat_food", vec![953]),
                TagEntry::new("minecraft:gold_ores", vec![70, 80, 71]),
                TagEntry::new("minecraft:gold_tool_materials", vec![909]),
                TagEntry::new("minecraft:grass_blocks", vec![27, 30, 423]),
                TagEntry::new(
                    "minecraft:hanging_signs",
                    vec![
                        1001, 1002, 1003, 1005, 1006, 1004, 1007, 1008, 1011, 1012, 1009, 1010,
                    ],
                ),
                TagEntry::new("minecraft:happy_ghast_food", vec![1017]),
                TagEntry::new(
                    "minecraft:happy_ghast_tempt_items",
                    vec![
                        1017, 839, 840, 841, 842, 843, 844, 845, 846, 847, 848, 849, 850, 851, 852,
                        853, 854,
                    ],
                ),
                TagEntry::new(
                    "minecraft:harnesses",
                    vec![
                        839, 840, 841, 842, 843, 844, 845, 846, 847, 848, 849, 850, 851, 852, 853,
                        854,
                    ],
                ),
                TagEntry::new(
                    "minecraft:head_armor",
                    vec![955, 959, 963, 975, 967, 971, 979, 888],
                ),
                TagEntry::new("minecraft:hoes", vec![941, 926, 931, 946, 916, 936, 921]),
                TagEntry::new("minecraft:hoglin_food", vec![250]),
                TagEntry::new(
                    "minecraft:horse_food",
                    vec![953, 1085, 505, 894, 1228, 1233, 987, 988],
                ),
                TagEntry::new("minecraft:horse_tempt_items", vec![1233, 987, 988]),
                TagEntry::new("minecraft:ignored_by_piglin_babies", vec![1018]),
                TagEntry::new("minecraft:iron_ores", vec![66, 67]),
                TagEntry::new("minecraft:iron_tool_materials", vec![905]),
                TagEntry::new("minecraft:jungle_logs", vec![137, 174, 151, 162]),
                TagEntry::new(
                    "minecraft:lanterns",
                    vec![1364, 1365, 1366, 1370, 1367, 1371, 1368, 1372, 1369, 1373],
                ),
                TagEntry::new("minecraft:lapis_ores", vec![76, 77]),
                TagEntry::new(
                    "minecraft:leaves",
                    vec![185, 182, 183, 189, 188, 186, 184, 191, 192, 190, 187],
                ),
                TagEntry::new("minecraft:lectern_books", vec![1222, 1221]),
                TagEntry::new(
                    "minecraft:leg_armor",
                    vec![957, 961, 965, 977, 969, 973, 981],
                ),
                TagEntry::new(
                    "minecraft:lightning_rods",
                    vec![734, 735, 736, 737, 738, 739, 740, 741],
                ),
                TagEntry::new("minecraft:llama_food", vec![953, 505]),
                TagEntry::new("minecraft:llama_tempt_items", vec![505]),
                TagEntry::new(
                    "minecraft:logs",
                    vec![
                        141, 178, 154, 165, 140, 177, 155, 166, 134, 171, 148, 159, 138, 175, 152,
                        163, 136, 173, 150, 161, 137, 174, 151, 162, 135, 172, 149, 160, 142, 179,
                        156, 167, 139, 176, 153, 164, 145, 157, 180, 168, 146, 158, 181, 169,
                    ],
                ),
                TagEntry::new(
                    "minecraft:logs_that_burn",
                    vec![
                        141, 178, 154, 165, 140, 177, 155, 166, 134, 171, 148, 159, 138, 175, 152,
                        163, 136, 173, 150, 161, 137, 174, 151, 162, 135, 172, 149, 160, 142, 179,
                        156, 167, 139, 176, 153, 164,
                    ],
                ),
                TagEntry::new(
                    "minecraft:loom_dyes",
                    vec![
                        1067, 1068, 1069, 1070, 1071, 1072, 1073, 1074, 1075, 1076, 1077, 1078,
                        1079, 1080, 1081, 1082,
                    ],
                ),
                TagEntry::new(
                    "minecraft:loom_patterns",
                    vec![1343, 1344, 1345, 1346, 1347, 1348, 1349, 1350, 1351, 1352],
                ),
                TagEntry::new("minecraft:mangrove_logs", vec![142, 179, 156, 167]),
                TagEntry::new("minecraft:map_invisibility_equipment", vec![358]),
                TagEntry::new(
                    "minecraft:meat",
                    vec![
                        1111, 1113, 1112, 1114, 1266, 985, 1251, 1265, 984, 1250, 1115,
                    ],
                ),
                TagEntry::new("minecraft:metal_nuggets", vec![1307, 1306, 1119]),
                TagEntry::new("minecraft:moss_blocks", vec![263, 266]),
                TagEntry::new("minecraft:mud", vec![32, 144]),
                TagEntry::new(
                    "minecraft:nautilus_bucket_food",
                    vec![1020, 1022, 1021, 1023],
                ),
                TagEntry::new(
                    "minecraft:nautilus_food",
                    vec![1058, 1062, 1059, 1063, 1061, 1060, 1020, 1022, 1021, 1023],
                ),
                TagEntry::new("minecraft:nautilus_taming_items", vec![1020, 1061]),
                TagEntry::new("minecraft:netherite_tool_materials", vec![910]),
                TagEntry::new(
                    "minecraft:non_flammable_wood",
                    vec![
                        146, 158, 181, 169, 145, 157, 180, 168, 46, 47, 282, 283, 778, 779, 355,
                        356, 812, 813, 832, 833, 453, 454, 762, 763, 791, 792, 999, 1000, 1012,
                        1011, 317, 310,
                    ],
                ),
                TagEntry::new(
                    "minecraft:noteblock_top_instruments",
                    vec![1237, 1234, 1238, 1239, 1235, 1240, 1236],
                ),
                TagEntry::new("minecraft:oak_logs", vec![134, 171, 148, 159]),
                TagEntry::new("minecraft:ocelot_food", vec![1058, 1059]),
                TagEntry::new("minecraft:pale_oak_logs", vec![140, 177, 155, 166]),
                TagEntry::new("minecraft:panda_eats_from_ground", vec![270, 1086]),
                TagEntry::new("minecraft:panda_food", vec![270]),
                TagEntry::new(
                    "minecraft:parrot_food",
                    vec![952, 1110, 1109, 1289, 1286, 1287],
                ),
                TagEntry::new("minecraft:parrot_poisonous_food", vec![1103]),
                TagEntry::new(
                    "minecraft:pickaxes",
                    vec![939, 924, 929, 944, 914, 934, 919],
                ),
                TagEntry::new("minecraft:pig_food", vec![1228, 1229, 1288]),
                TagEntry::new("minecraft:piglin_food", vec![984, 985]),
                TagEntry::new(
                    "minecraft:piglin_loved",
                    vec![
                        70, 80, 71, 92, 1389, 766, 909, 1363, 1055, 1233, 1130, 987, 988, 975, 976,
                        977, 978, 1258, 1335, 927, 1301, 929, 928, 930, 931, 908, 86, 230,
                    ],
                ),
                TagEntry::new("minecraft:piglin_preferred_weapons", vec![1340, 1301]),
                TagEntry::new("minecraft:piglin_repellents", vec![366, 1365, 1377]),
                TagEntry::new("minecraft:piglin_safe_armor", vec![975, 976, 977, 978]),
                TagEntry::new("minecraft:pillager_preferred_weapons", vec![1340]),
                TagEntry::new(
                    "minecraft:planks",
                    vec![36, 37, 38, 39, 40, 42, 43, 46, 47, 44, 45, 41],
                ),
                TagEntry::new("minecraft:rabbit_food", vec![1228, 1233, 229]),
                TagEntry::new("minecraft:rails", vec![836, 834, 835, 837]),
                TagEntry::new("minecraft:redstone_ores", vec![72, 73]),
                TagEntry::new("minecraft:repairs_chain_armor", vec![905]),
                TagEntry::new("minecraft:repairs_copper_armor", vec![907]),
                TagEntry::new("minecraft:repairs_diamond_armor", vec![899]),
                TagEntry::new("minecraft:repairs_gold_armor", vec![909]),
                TagEntry::new("minecraft:repairs_iron_armor", vec![905]),
                TagEntry::new("minecraft:repairs_leather_armor", vec![1018]),
                TagEntry::new("minecraft:repairs_netherite_armor", vec![910]),
                TagEntry::new("minecraft:repairs_turtle_helmet", vec![889]),
                TagEntry::new("minecraft:repairs_wolf_armor", vec![890]),
                TagEntry::new("minecraft:sand", vec![59, 62, 60]),
                TagEntry::new(
                    "minecraft:saplings",
                    vec![49, 50, 51, 52, 53, 55, 56, 205, 206, 57, 54],
                ),
                TagEntry::new("minecraft:shearable_from_copper_golem", vec![233]),
                TagEntry::new("minecraft:sheep_food", vec![953]),
                TagEntry::new("minecraft:shovels", vec![938, 923, 928, 943, 913, 933, 918]),
                TagEntry::new(
                    "minecraft:shulker_boxes",
                    vec![
                        582, 598, 594, 595, 592, 590, 596, 586, 591, 588, 585, 584, 589, 593, 597,
                        583, 587,
                    ],
                ),
                TagEntry::new(
                    "minecraft:signs",
                    vec![989, 990, 991, 993, 992, 995, 996, 999, 1000, 997, 998, 994],
                ),
                TagEntry::new("minecraft:skeleton_preferred_weapons", vec![895]),
                TagEntry::new(
                    "minecraft:skulls",
                    vec![1236, 1238, 1237, 1234, 1235, 1239, 1240],
                ),
                TagEntry::new(
                    "minecraft:slabs",
                    vec![
                        271, 272, 273, 274, 275, 277, 278, 282, 283, 279, 280, 276, 281, 284, 285,
                        291, 286, 297, 294, 295, 290, 289, 293, 288, 298, 299, 300, 700, 701, 702,
                        703, 704, 705, 706, 707, 708, 709, 710, 711, 712, 287, 296, 1387, 1395,
                        1391, 713, 714, 716, 715, 132, 131, 130, 113, 112, 111, 110, 133, 292, 13,
                        18, 22, 417,
                    ],
                ),
                TagEntry::new(
                    "minecraft:small_flowers",
                    vec![
                        229, 231, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245,
                        232, 230,
                    ],
                ),
                TagEntry::new("minecraft:smelts_to_glass", vec![59, 62]),
                TagEntry::new("minecraft:sniffer_food", vec![1286]),
                TagEntry::new("minecraft:soul_fire_base_blocks", vec![361, 362]),
                TagEntry::new(
                    "minecraft:spears",
                    vec![1302, 1298, 1301, 1303, 1297, 1300, 1299],
                ),
                TagEntry::new("minecraft:spruce_logs", vec![135, 172, 149, 160]),
                TagEntry::new(
                    "minecraft:stairs",
                    vec![
                        442, 443, 444, 445, 446, 448, 449, 453, 454, 450, 451, 447, 452, 337, 439,
                        429, 421, 420, 329, 486, 573, 567, 566, 568, 682, 683, 684, 685, 686, 687,
                        688, 689, 690, 691, 692, 693, 694, 695, 1388, 1396, 1392, 696, 697, 699,
                        698, 109, 108, 107, 106, 128, 127, 126, 129, 422, 14, 19, 23, 416,
                    ],
                ),
                TagEntry::new("minecraft:stone_bricks", vec![376, 377, 378, 379]),
                TagEntry::new("minecraft:stone_buttons", vec![750, 751]),
                TagEntry::new("minecraft:stone_crafting_materials", vec![35, 1386, 9]),
                TagEntry::new("minecraft:stone_tool_materials", vec![35, 1386, 9]),
                TagEntry::new("minecraft:strider_food", vec![251]),
                TagEntry::new("minecraft:strider_tempt_items", vec![251, 861]),
                TagEntry::new("minecraft:swords", vec![937, 922, 927, 942, 912, 932, 917]),
                TagEntry::new(
                    "minecraft:terracotta",
                    vec![
                        522, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498, 499, 500,
                        501, 502,
                    ],
                ),
                TagEntry::new(
                    "minecraft:trapdoors",
                    vec![
                        806, 804, 808, 809, 805, 802, 803, 812, 813, 810, 811, 807, 801, 814, 815,
                        816, 817, 818, 819, 820, 821,
                    ],
                ),
                TagEntry::new(
                    "minecraft:trim_materials",
                    vec![903, 907, 899, 900, 909, 905, 901, 910, 902, 718, 1247],
                ),
                TagEntry::new(
                    "minecraft:trimmable_armor",
                    vec![
                        958, 962, 966, 978, 970, 974, 982, 957, 961, 965, 977, 969, 973, 981, 956,
                        960, 964, 976, 968, 972, 980, 955, 959, 963, 975, 967, 971, 979, 888,
                    ],
                ),
                TagEntry::new("minecraft:turtle_food", vec![211]),
                TagEntry::new(
                    "minecraft:villager_picks_up",
                    vec![952, 1229, 1228, 1289, 1286, 1287, 954, 953, 1288],
                ),
                TagEntry::new(
                    "minecraft:villager_plantable_seeds",
                    vec![952, 1229, 1228, 1289, 1286, 1287],
                ),
                TagEntry::new(
                    "minecraft:walls",
                    vec![
                        457, 458, 459, 460, 461, 462, 463, 464, 466, 467, 468, 469, 470, 471, 472,
                        474, 473, 475, 476, 478, 477, 465, 15, 20, 24, 418,
                    ],
                ),
                TagEntry::new("minecraft:warped_stems", vec![146, 158, 181, 169]),
                TagEntry::new("minecraft:wart_blocks", vec![577, 578]),
                TagEntry::new(
                    "minecraft:wither_skeleton_disliked_weapons",
                    vec![895, 1340],
                ),
                TagEntry::new(
                    "minecraft:wolf_collar_dyes",
                    vec![
                        1067, 1068, 1069, 1070, 1071, 1072, 1073, 1074, 1075, 1076, 1077, 1078,
                        1079, 1080, 1081, 1082,
                    ],
                ),
                TagEntry::new(
                    "minecraft:wolf_food",
                    vec![
                        1111, 1113, 1112, 1114, 1266, 985, 1251, 1265, 984, 1250, 1115, 1058, 1062,
                        1059, 1063, 1060, 1061, 1252,
                    ],
                ),
                TagEntry::new(
                    "minecraft:wooden_buttons",
                    vec![752, 753, 754, 755, 756, 758, 759, 762, 763, 760, 761, 757],
                ),
                TagEntry::new(
                    "minecraft:wooden_doors",
                    vec![781, 782, 783, 784, 785, 787, 788, 791, 792, 789, 790, 786],
                ),
                TagEntry::new(
                    "minecraft:wooden_fences",
                    vec![345, 349, 351, 352, 346, 347, 348, 355, 356, 353, 354, 350],
                ),
                TagEntry::new(
                    "minecraft:wooden_pressure_plates",
                    vec![768, 769, 770, 771, 772, 774, 775, 778, 779, 776, 777, 773],
                ),
                TagEntry::new(
                    "minecraft:wooden_shelves",
                    vec![306, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316, 317],
                ),
                TagEntry::new(
                    "minecraft:wooden_slabs",
                    vec![271, 272, 273, 274, 275, 277, 278, 282, 283, 279, 280, 276],
                ),
                TagEntry::new(
                    "minecraft:wooden_stairs",
                    vec![442, 443, 444, 445, 446, 448, 449, 453, 454, 450, 451, 447],
                ),
                TagEntry::new(
                    "minecraft:wooden_tool_materials",
                    vec![36, 37, 38, 39, 40, 42, 43, 46, 47, 44, 45, 41],
                ),
                TagEntry::new(
                    "minecraft:wooden_trapdoors",
                    vec![806, 804, 808, 809, 805, 802, 803, 812, 813, 810, 811, 807],
                ),
                TagEntry::new(
                    "minecraft:wool",
                    vec![
                        213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227,
                        228,
                    ],
                ),
                TagEntry::new(
                    "minecraft:wool_carpets",
                    vec![
                        506, 507, 508, 509, 510, 511, 512, 513, 514, 515, 516, 517, 518, 519, 520,
                        521,
                    ],
                ),
                TagEntry::new("minecraft:zombie_horse_food", vec![249]),
            ],
        ),
        TagRegistry::new(
            "minecraft:painting_variant",
            vec![TagEntry::new(
                "minecraft:placeable",
                vec![
                    24, 1, 0, 2, 5, 32, 47, 35, 12, 37, 42, 13, 46, 22, 26, 8, 40, 45, 39, 50, 19,
                    33, 31, 7, 38, 15, 4, 23, 27, 36, 44, 3, 6, 9, 10, 11, 17, 18, 20, 25, 28, 29,
                    30, 34, 41, 43, 14,
                ],
            )],
        ),
        TagRegistry::new(
            "minecraft:point_of_interest_type",
            vec![
                TagEntry::new(
                    "minecraft:acquirable_job_site",
                    vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
                ),
                TagEntry::new("minecraft:bee_home", vec![15, 16]),
                TagEntry::new(
                    "minecraft:village",
                    vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
                ),
            ],
        ),
        TagRegistry::new(
            "minecraft:potion",
            vec![TagEntry::new(
                "minecraft:tradeable",
                vec![
                    42, 44, 45, 43, 4, 5, 6, 7, 11, 12, 8, 9, 10, 16, 17, 18, 19, 20, 21, 13, 14,
                    15, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 40, 41,
                ],
            )],
        ),
        TagRegistry::new(
            "minecraft:timeline",
            vec![
                TagEntry::new("minecraft:in_end", vec![3]),
                TagEntry::new("minecraft:in_nether", vec![3]),
                TagEntry::new("minecraft:in_overworld", vec![3, 0, 2, 1]),
                TagEntry::new("minecraft:universal", vec![3]),
            ],
        ),
        TagRegistry::new(
            "minecraft:villager_trade",
            vec![
                TagEntry::new("minecraft:armorer/level_1", vec![269, 3, 0, 2, 1]),
                TagEntry::new("minecraft:armorer/level_2", vec![271, 270, 4, 5]),
                TagEntry::new("minecraft:armorer/level_3", vec![10, 8, 7, 9, 6]),
                TagEntry::new("minecraft:armorer/level_4", vec![12, 11]),
                TagEntry::new("minecraft:armorer/level_5", vec![14, 13]),
                TagEntry::new("minecraft:butcher/level_1", vec![15, 17, 18, 16]),
                TagEntry::new("minecraft:butcher/level_2", vec![19, 21, 20]),
                TagEntry::new("minecraft:butcher/level_3", vec![23, 22]),
                TagEntry::new("minecraft:butcher/level_4", vec![24]),
                TagEntry::new("minecraft:butcher/level_5", vec![25]),
                TagEntry::new("minecraft:cartographer/level_1", vec![27, 26]),
                TagEntry::new(
                    "minecraft:cartographer/level_2",
                    vec![35, 34, 29, 33, 32, 31, 28, 30],
                ),
                TagEntry::new("minecraft:cartographer/level_3", vec![36, 37, 38]),
                TagEntry::new(
                    "minecraft:cartographer/level_4",
                    vec![
                        45, 53, 49, 48, 40, 46, 54, 47, 50, 43, 42, 51, 41, 44, 52, 39,
                    ],
                ),
                TagEntry::new("minecraft:cartographer/level_5", vec![56, 55]),
                TagEntry::new("minecraft:cleric/level_1", vec![58, 57]),
                TagEntry::new("minecraft:cleric/level_2", vec![60, 59]),
                TagEntry::new("minecraft:cleric/level_3", vec![62, 61]),
                TagEntry::new("minecraft:cleric/level_4", vec![65, 64, 63]),
                TagEntry::new("minecraft:cleric/level_5", vec![67, 66]),
                TagEntry::new("minecraft:common_smith/level_1", vec![269]),
                TagEntry::new("minecraft:common_smith/level_2", vec![271, 270]),
                TagEntry::new("minecraft:common_smith/level_3", vec![]),
                TagEntry::new("minecraft:common_smith/level_4", vec![]),
                TagEntry::new("minecraft:common_smith/level_5", vec![]),
                TagEntry::new("minecraft:farmer/level_1", vec![72, 71, 69, 68, 70]),
                TagEntry::new("minecraft:farmer/level_2", vec![75, 74, 73]),
                TagEntry::new("minecraft:farmer/level_3", vec![76, 77]),
                TagEntry::new("minecraft:farmer/level_4", vec![78, 79]),
                TagEntry::new("minecraft:farmer/level_5", vec![81, 80]),
                TagEntry::new("minecraft:fisherman/level_1", vec![85, 82, 84, 83]),
                TagEntry::new("minecraft:fisherman/level_2", vec![86, 88, 87]),
                TagEntry::new("minecraft:fisherman/level_3", vec![90, 89]),
                TagEntry::new("minecraft:fisherman/level_4", vec![91]),
                TagEntry::new("minecraft:fisherman/level_5", vec![96, 95, 97, 94, 92, 93]),
                TagEntry::new("minecraft:fletcher/level_1", vec![100, 98, 99]),
                TagEntry::new("minecraft:fletcher/level_2", vec![102, 101]),
                TagEntry::new("minecraft:fletcher/level_3", vec![104, 103]),
                TagEntry::new("minecraft:fletcher/level_4", vec![106, 105]),
                TagEntry::new("minecraft:fletcher/level_5", vec![109, 108, 107]),
                TagEntry::new("minecraft:leatherworker/level_1", vec![112, 111, 110]),
                TagEntry::new("minecraft:leatherworker/level_2", vec![115, 114, 113]),
                TagEntry::new("minecraft:leatherworker/level_3", vec![117, 116]),
                TagEntry::new("minecraft:leatherworker/level_4", vec![119, 118]),
                TagEntry::new("minecraft:leatherworker/level_5", vec![121, 120]),
                TagEntry::new("minecraft:librarian/level_1", vec![124, 122, 123]),
                TagEntry::new("minecraft:librarian/level_2", vec![125, 126, 127]),
                TagEntry::new("minecraft:librarian/level_3", vec![130, 128, 129]),
                TagEntry::new("minecraft:librarian/level_4", vec![134, 131, 132, 133]),
                TagEntry::new("minecraft:librarian/level_5", vec![136, 135]),
                TagEntry::new("minecraft:mason/level_1", vec![137, 138]),
                TagEntry::new("minecraft:mason/level_2", vec![140, 139]),
                TagEntry::new(
                    "minecraft:mason/level_3",
                    vec![147, 141, 142, 143, 144, 145, 146],
                ),
                TagEntry::new(
                    "minecraft:mason/level_4",
                    vec![
                        180, 169, 177, 151, 161, 157, 163, 149, 175, 171, 167, 165, 159, 155, 173,
                        179, 153, 168, 176, 150, 160, 156, 162, 148, 174, 170, 166, 164, 158, 154,
                        172, 178, 152,
                    ],
                ),
                TagEntry::new("minecraft:mason/level_5", vec![182, 181]),
                TagEntry::new("minecraft:shepherd/level_1", vec![187, 184, 186, 183, 185]),
                TagEntry::new(
                    "minecraft:shepherd/level_2",
                    vec![
                        224, 221, 188, 222, 223, 218, 210, 208, 192, 202, 220, 206, 212, 198, 204,
                        196, 214, 194, 200, 216, 190, 217, 209, 207, 191, 201, 219, 205, 211, 197,
                        203, 195, 213, 193, 199, 215, 189,
                    ],
                ),
                TagEntry::new(
                    "minecraft:shepherd/level_3",
                    vec![
                        245, 241, 242, 244, 243, 239, 235, 234, 226, 231, 240, 233, 236, 229, 232,
                        228, 237, 227, 230, 238, 225,
                    ],
                ),
                TagEntry::new(
                    "minecraft:shepherd/level_4",
                    vec![
                        247, 267, 246, 265, 266, 248, 263, 259, 258, 250, 255, 264, 257, 260, 253,
                        256, 252, 261, 251, 254, 262, 249,
                    ],
                ),
                TagEntry::new("minecraft:shepherd/level_5", vec![268]),
                TagEntry::new("minecraft:toolsmith/level_1", vec![269, 272, 275, 274, 273]),
                TagEntry::new("minecraft:toolsmith/level_2", vec![271, 270]),
                TagEntry::new("minecraft:toolsmith/level_3", vec![280, 277, 279, 278, 276]),
                TagEntry::new("minecraft:toolsmith/level_4", vec![282, 283, 281]),
                TagEntry::new("minecraft:toolsmith/level_5", vec![284]),
                TagEntry::new(
                    "minecraft:wandering_trader/buying",
                    vec![379, 380, 378, 376, 285, 377],
                ),
                TagEntry::new(
                    "minecraft:wandering_trader/common",
                    vec![
                        314, 353, 363, 364, 315, 336, 311, 368, 354, 323, 301, 306, 352, 296, 288,
                        289, 360, 341, 373, 349, 342, 304, 326, 339, 371, 290, 355, 333, 287, 292,
                        308, 322, 338, 367, 303, 347, 332, 357, 372, 294, 348, 293, 318, 325, 330,
                        375, 317, 356, 324, 328, 340, 298, 305, 297, 300, 312, 320, 369, 370, 344,
                        299, 358, 327, 365, 362, 359, 351, 361, 334, 345, 374, 309, 313, 316, 335,
                    ],
                ),
                TagEntry::new(
                    "minecraft:wandering_trader/uncommon",
                    vec![
                        343, 295, 319, 350, 286, 291, 307, 321, 337, 366, 302, 331, 346, 310, 329,
                    ],
                ),
                TagEntry::new("minecraft:weaponsmith/level_1", vec![269, 382, 381]),
                TagEntry::new("minecraft:weaponsmith/level_2", vec![271, 270]),
                TagEntry::new("minecraft:weaponsmith/level_3", vec![383]),
                TagEntry::new("minecraft:weaponsmith/level_4", vec![385, 384]),
                TagEntry::new("minecraft:weaponsmith/level_5", vec![386]),
            ],
        ),
        TagRegistry::new(
            "minecraft:worldgen/biome",
            vec![
                TagEntry::new("minecraft:allows_surface_slime_spawns", vec![54, 31]),
                TagEntry::new(
                    "minecraft:allows_tropical_fish_spawns_at_any_height",
                    vec![30],
                ),
                TagEntry::new("minecraft:has_structure/ancient_city", vec![10]),
                TagEntry::new(
                    "minecraft:has_structure/bastion_remnant",
                    vec![7, 34, 49, 59],
                ),
                TagEntry::new("minecraft:has_structure/buried_treasure", vec![3, 45]),
                TagEntry::new("minecraft:has_structure/desert_pyramid", vec![14]),
                TagEntry::new("minecraft:has_structure/end_city", vec![17, 18]),
                TagEntry::new("minecraft:has_structure/igloo", vec![48, 46, 47]),
                TagEntry::new("minecraft:has_structure/jungle_temple", vec![1, 28]),
                TagEntry::new(
                    "minecraft:has_structure/mineshaft",
                    vec![
                        11, 9, 13, 12, 22, 35, 6, 29, 58, 41, 24, 3, 45, 32, 23, 27, 51, 47, 5, 62,
                        60, 61, 55, 48, 37, 38, 1, 28, 50, 21, 20, 4, 36, 8, 39, 25, 52, 33, 26,
                        63, 14, 42, 46, 40, 53, 54, 31, 43, 15, 30,
                    ],
                ),
                TagEntry::new("minecraft:has_structure/mineshaft_mesa", vec![0, 19, 64]),
                TagEntry::new(
                    "minecraft:has_structure/nether_fortress",
                    vec![34, 49, 7, 59, 2],
                ),
                TagEntry::new("minecraft:has_structure/nether_fossil", vec![49]),
                TagEntry::new(
                    "minecraft:has_structure/ocean_monument",
                    vec![11, 9, 13, 12],
                ),
                TagEntry::new(
                    "minecraft:has_structure/ocean_ruin_cold",
                    vec![22, 6, 35, 11, 9, 13],
                ),
                TagEntry::new("minecraft:has_structure/ocean_ruin_warm", vec![29, 58, 12]),
                TagEntry::new(
                    "minecraft:has_structure/pillager_outpost",
                    vec![14, 40, 42, 46, 55, 32, 23, 27, 51, 47, 5, 25],
                ),
                TagEntry::new("minecraft:has_structure/ruined_portal_desert", vec![14]),
                TagEntry::new(
                    "minecraft:has_structure/ruined_portal_jungle",
                    vec![1, 28, 50],
                ),
                TagEntry::new(
                    "minecraft:has_structure/ruined_portal_mountain",
                    vec![0, 19, 64, 62, 60, 61, 43, 63, 52, 32, 23, 27, 51, 47, 5],
                ),
                TagEntry::new(
                    "minecraft:has_structure/ruined_portal_nether",
                    vec![34, 49, 7, 59, 2],
                ),
                TagEntry::new(
                    "minecraft:has_structure/ruined_portal_ocean",
                    vec![11, 9, 13, 12, 22, 35, 6, 29, 58],
                ),
                TagEntry::new(
                    "minecraft:has_structure/ruined_portal_standard",
                    vec![
                        3, 45, 41, 24, 55, 48, 37, 38, 21, 20, 4, 36, 8, 39, 25, 33, 26, 15, 30,
                        42, 46, 40, 53,
                    ],
                ),
                TagEntry::new("minecraft:has_structure/ruined_portal_swamp", vec![54, 31]),
                TagEntry::new(
                    "minecraft:has_structure/shipwreck",
                    vec![11, 9, 13, 12, 22, 35, 6, 29, 58],
                ),
                TagEntry::new("minecraft:has_structure/shipwreck_beached", vec![3, 45]),
                TagEntry::new(
                    "minecraft:has_structure/stronghold",
                    vec![
                        33, 11, 22, 9, 6, 13, 35, 12, 29, 58, 52, 54, 31, 47, 46, 45, 61, 25, 62,
                        48, 60, 55, 40, 32, 3, 21, 38, 20, 4, 8, 39, 43, 42, 28, 0, 14, 64, 27, 51,
                        24, 41, 26, 37, 53, 36, 50, 1, 19, 63, 5, 23, 15, 30, 10,
                    ],
                ),
                TagEntry::new("minecraft:has_structure/swamp_hut", vec![54]),
                TagEntry::new(
                    "minecraft:has_structure/trail_ruins",
                    vec![55, 48, 37, 38, 36, 28],
                ),
                TagEntry::new(
                    "minecraft:has_structure/trial_chambers",
                    vec![
                        33, 11, 22, 9, 6, 13, 35, 12, 29, 58, 52, 54, 31, 47, 46, 45, 61, 25, 62,
                        48, 60, 55, 40, 32, 3, 21, 38, 20, 4, 8, 39, 43, 42, 28, 0, 14, 64, 27, 51,
                        24, 41, 26, 37, 53, 36, 50, 1, 19, 63, 5, 23, 15, 30,
                    ],
                ),
                TagEntry::new("minecraft:has_structure/village_desert", vec![14]),
                TagEntry::new("minecraft:has_structure/village_plains", vec![40, 32]),
                TagEntry::new("minecraft:has_structure/village_savanna", vec![42]),
                TagEntry::new("minecraft:has_structure/village_snowy", vec![46]),
                TagEntry::new("minecraft:has_structure/village_taiga", vec![55]),
                TagEntry::new("minecraft:has_structure/woodland_mansion", vec![8, 39]),
                TagEntry::new("minecraft:is_badlands", vec![0, 19, 64]),
                TagEntry::new("minecraft:is_beach", vec![3, 45]),
                TagEntry::new("minecraft:is_deep_ocean", vec![11, 9, 13, 12]),
                TagEntry::new("minecraft:is_end", vec![56, 17, 18, 44, 16]),
                TagEntry::new("minecraft:is_forest", vec![21, 20, 4, 36, 8, 39, 25]),
                TagEntry::new("minecraft:is_hill", vec![62, 60, 61]),
                TagEntry::new("minecraft:is_jungle", vec![1, 28, 50]),
                TagEntry::new("minecraft:is_mountain", vec![32, 23, 27, 51, 47, 5]),
                TagEntry::new("minecraft:is_nether", vec![34, 49, 7, 59, 2]),
                TagEntry::new("minecraft:is_ocean", vec![11, 9, 13, 12, 22, 35, 6, 29, 58]),
                TagEntry::new(
                    "minecraft:is_overworld",
                    vec![
                        33, 11, 22, 9, 6, 13, 35, 12, 29, 58, 52, 54, 31, 47, 46, 45, 61, 25, 62,
                        48, 60, 55, 40, 32, 3, 21, 38, 20, 4, 8, 39, 43, 42, 28, 0, 14, 64, 27, 51,
                        24, 41, 26, 37, 53, 36, 50, 1, 19, 63, 5, 23, 15, 30, 10,
                    ],
                ),
                TagEntry::new("minecraft:is_river", vec![41, 24]),
                TagEntry::new("minecraft:is_savanna", vec![42, 43, 63]),
                TagEntry::new("minecraft:is_taiga", vec![55, 48, 37, 38]),
                TagEntry::new("minecraft:mineshaft_blocking", vec![10]),
                TagEntry::new("minecraft:more_frequent_drowned_spawns", vec![41, 24]),
                TagEntry::new(
                    "minecraft:polar_bears_spawn_on_alternate_blocks",
                    vec![22, 11],
                ),
                TagEntry::new("minecraft:produces_corals_from_bonemeal", vec![58]),
                TagEntry::new("minecraft:reduce_water_ambient_spawns", vec![41, 24]),
                TagEntry::new(
                    "minecraft:required_ocean_monument_surrounding",
                    vec![11, 9, 13, 12, 22, 35, 6, 29, 58, 41, 24],
                ),
                TagEntry::new(
                    "minecraft:spawns_cold_variant_farm_animals",
                    vec![
                        46, 26, 23, 27, 47, 22, 11, 25, 10, 24, 48, 45, 56, 17, 18, 44, 16, 6, 9,
                        37, 38, 55, 60, 61, 62, 51,
                    ],
                ),
                TagEntry::new(
                    "minecraft:spawns_cold_variant_frogs",
                    vec![
                        46, 26, 23, 27, 47, 22, 11, 25, 10, 24, 48, 45, 56, 17, 18, 44, 16,
                    ],
                ),
                TagEntry::new("minecraft:spawns_coral_variant_zombie_nautilus", vec![58]),
                TagEntry::new("minecraft:spawns_gold_rabbits", vec![14]),
                TagEntry::new(
                    "minecraft:spawns_snow_foxes",
                    vec![46, 26, 22, 48, 24, 45, 23, 27, 47, 25],
                ),
                TagEntry::new(
                    "minecraft:spawns_warm_variant_farm_animals",
                    vec![
                        14, 58, 1, 28, 50, 42, 43, 63, 34, 49, 7, 59, 2, 0, 19, 64, 31, 12, 29,
                    ],
                ),
                TagEntry::new(
                    "minecraft:spawns_warm_variant_frogs",
                    vec![
                        14, 58, 1, 28, 50, 42, 43, 63, 34, 49, 7, 59, 2, 0, 19, 64, 31,
                    ],
                ),
                TagEntry::new(
                    "minecraft:spawns_white_rabbits",
                    vec![46, 26, 22, 48, 24, 45, 23, 27, 47, 25],
                ),
                TagEntry::new(
                    "minecraft:stronghold_biased_to",
                    vec![
                        40, 53, 46, 26, 14, 21, 20, 4, 8, 39, 36, 37, 38, 55, 48, 42, 43, 62, 61,
                        60, 63, 28, 50, 1, 0, 19, 64, 32, 5, 25, 47, 23, 27, 51, 33, 15, 30,
                    ],
                ),
                TagEntry::new(
                    "minecraft:water_on_map_outlines",
                    vec![11, 9, 13, 12, 22, 35, 6, 29, 58, 41, 24, 54, 31],
                ),
                TagEntry::new("minecraft:without_wandering_trader_spawns", vec![57]),
                TagEntry::new("minecraft:without_zombie_sieges", vec![33]),
            ],
        ),
        TagRegistry::new(
            "minecraft:worldgen/configured_feature",
            vec![TagEntry::new(
                "minecraft:can_spawn_from_bone_meal",
                vec![65, 66, 70, 69, 67, 64, 220, 68],
            )],
        ),
        TagRegistry::new(
            "minecraft:worldgen/flat_level_generator_preset",
            vec![TagEntry::new(
                "minecraft:visible",
                vec![1, 7, 8, 3, 5, 0, 2, 4, 6],
            )],
        ),
        TagRegistry::new(
            "minecraft:worldgen/structure",
            vec![
                TagEntry::new("minecraft:cats_spawn_as_black", vec![26]),
                TagEntry::new("minecraft:cats_spawn_in", vec![26]),
                TagEntry::new("minecraft:dolphin_located", vec![13, 14, 23, 24]),
                TagEntry::new("minecraft:eye_of_ender_located", vec![25]),
                TagEntry::new("minecraft:mineshaft", vec![9, 10]),
                TagEntry::new("minecraft:ocean_ruin", vec![13, 14]),
                TagEntry::new("minecraft:on_desert_village_maps", vec![29]),
                TagEntry::new("minecraft:on_jungle_explorer_maps", vec![7]),
                TagEntry::new("minecraft:on_ocean_explorer_maps", vec![11]),
                TagEntry::new("minecraft:on_plains_village_maps", vec![30]),
                TagEntry::new("minecraft:on_savanna_village_maps", vec![31]),
                TagEntry::new("minecraft:on_snowy_village_maps", vec![32]),
                TagEntry::new("minecraft:on_swamp_explorer_maps", vec![26]),
                TagEntry::new("minecraft:on_taiga_village_maps", vec![33]),
                TagEntry::new("minecraft:on_treasure_maps", vec![2]),
                TagEntry::new("minecraft:on_trial_chambers_maps", vec![28]),
                TagEntry::new("minecraft:on_woodland_explorer_maps", vec![8]),
                TagEntry::new("minecraft:ruined_portal", vec![17, 18, 19, 20, 21, 16, 22]),
                TagEntry::new("minecraft:shipwreck", vec![23, 24]),
                TagEntry::new("minecraft:village", vec![30, 29, 31, 32, 33]),
            ],
        ),
        TagRegistry::new(
            "minecraft:worldgen/world_preset",
            vec![
                TagEntry::new("minecraft:extended", vec![4, 2, 3, 0, 5, 1]),
                TagEntry::new("minecraft:normal", vec![4, 2, 3, 0, 5]),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tag_counts_match_the_manifest() {
        let registries = configuration_tags();
        assert_eq!(registries.len(), TAG_REGISTRY_COUNT);
        assert_eq!(
            registries
                .iter()
                .map(|registry| registry.tags.len())
                .sum::<usize>(),
            TAG_COUNT
        );
        assert_eq!(
            registries
                .iter()
                .flat_map(|registry| &registry.tags)
                .map(|tag| tag.entries.len())
                .sum::<usize>(),
            TAG_ENTRY_COUNT
        );
    }

    #[test]
    fn required_gameplay_tag_registries_are_present() {
        let registries = configuration_tags();
        for id in [
            "minecraft:block",
            "minecraft:item",
            "minecraft:entity_type",
            "minecraft:worldgen/biome",
        ] {
            assert!(registries.iter().any(|registry| registry.id == id));
        }
    }
}
