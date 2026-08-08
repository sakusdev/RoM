use ferrum_game::Experience;
use thiserror::Error;

pub fn encode_set_experience(experience: Experience) -> Result<Vec<u8>, ExperienceEncodeError> {
    if !experience.progress.is_finite() || !(0.0..1.0).contains(&experience.progress) {
        return Err(ExperienceEncodeError::InvalidProgress {
            progress: experience.progress,
        });
    }
    let level =
        i32::try_from(experience.level).map_err(|_| ExperienceEncodeError::LevelOutOfRange {
            level: experience.level,
        })?;
    let total =
        i32::try_from(experience.total).map_err(|_| ExperienceEncodeError::TotalOutOfRange {
            total: experience.total,
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

#[derive(Debug, Error, PartialEq)]
pub enum ExperienceEncodeError {
    #[error("experience progress {progress} must be finite and in 0..1")]
    InvalidProgress { progress: f32 },
    #[error("experience level {level} exceeds the protocol VarInt range")]
    LevelOutOfRange { level: u32 },
    #[error("total experience {total} exceeds the protocol VarInt range")]
    TotalOutOfRange { total: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_official_progress_level_total_field_order() {
        assert_eq!(
            encode_set_experience(Experience {
                level: 12,
                progress: 0.5,
                total: 300,
            })
            .unwrap(),
            vec![0x3f, 0, 0, 0, 0x0c, 0xac, 0x02]
        );
    }

    #[test]
    fn rejects_non_canonical_experience_values() {
        assert!(matches!(
            encode_set_experience(Experience {
                progress: 1.0,
                ..Experience::default()
            }),
            Err(ExperienceEncodeError::InvalidProgress { .. })
        ));
        assert_eq!(
            encode_set_experience(Experience {
                level: 0,
                progress: 0.0,
                total: i32::MAX as u64 + 1,
            }),
            Err(ExperienceEncodeError::TotalOutOfRange {
                total: i32::MAX as u64 + 1,
            })
        );
    }
}
