use rom_game::Experience;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ExperienceEncodeError {
    #[error("experience progress must be finite and in 0..=1: {value}")]
    InvalidProgress { value: f32 },
    #[error("experience level {value} exceeds the protocol VarInt range")]
    LevelOutOfRange { value: u32 },
    #[error("total experience {value} exceeds the protocol VarInt range")]
    TotalOutOfRange { value: u64 },
}

/// Encodes the 26.x clientbound Set Experience payload.
///
/// The vanilla wire order is progress (Float), level (VarInt), total (VarInt).
pub fn encode_set_experience(experience: Experience) -> Result<Vec<u8>, ExperienceEncodeError> {
    if !experience.progress.is_finite() || !(0.0..=1.0).contains(&experience.progress) {
        return Err(ExperienceEncodeError::InvalidProgress {
            value: experience.progress,
        });
    }
    let level =
        i32::try_from(experience.level).map_err(|_| ExperienceEncodeError::LevelOutOfRange {
            value: experience.level,
        })?;
    let total =
        i32::try_from(experience.total).map_err(|_| ExperienceEncodeError::TotalOutOfRange {
            value: experience.total,
        })?;

    let mut output = Vec::with_capacity(14);
    output.extend_from_slice(&experience.progress.to_be_bytes());
    write_varint(&mut output, level);
    write_varint(&mut output, total);
    Ok(output)
}

fn write_varint(output: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        if value & !0x7f == 0 {
            output.push(value as u8);
            return;
        }
        output.push(((value & 0x7f) | 0x80) as u8);
        value >>= 7;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_progress_level_and_total_in_vanilla_order() {
        let payload = encode_set_experience(Experience {
            level: 300,
            progress: 0.5,
            total: 12_345,
        })
        .unwrap();
        assert_eq!(
            payload,
            vec![0x3f, 0x00, 0x00, 0x00, 0xac, 0x02, 0xb9, 0x60]
        );
    }

    #[test]
    fn validates_progress_and_varint_ranges() {
        assert!(matches!(
            encode_set_experience(Experience {
                progress: f32::NAN,
                ..Experience::default()
            }),
            Err(ExperienceEncodeError::InvalidProgress { .. })
        ));
        assert_eq!(
            encode_set_experience(Experience {
                level: i32::MAX as u32 + 1,
                ..Experience::default()
            }),
            Err(ExperienceEncodeError::LevelOutOfRange {
                value: i32::MAX as u32 + 1,
            })
        );
        assert_eq!(
            encode_set_experience(Experience {
                total: i32::MAX as u64 + 1,
                ..Experience::default()
            }),
            Err(ExperienceEncodeError::TotalOutOfRange {
                value: i32::MAX as u64 + 1,
            })
        );
    }
}
