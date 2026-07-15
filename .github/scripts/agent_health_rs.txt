use ferrum_game::Vitals;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum HealthEncodeError {
    #[error("health must be finite and non-negative: {value}")]
    InvalidHealth { value: f32 },
    #[error("saturation must be finite and non-negative: {value}")]
    InvalidSaturation { value: f32 },
}

pub fn encode_set_health(vitals: Vitals) -> Result<Vec<u8>, HealthEncodeError> {
    if !vitals.health.is_finite() || vitals.health < 0.0 {
        return Err(HealthEncodeError::InvalidHealth {
            value: vitals.health,
        });
    }
    if !vitals.saturation.is_finite() || vitals.saturation < 0.0 {
        return Err(HealthEncodeError::InvalidSaturation {
            value: vitals.saturation,
        });
    }
    let mut output = Vec::with_capacity(10);
    output.extend_from_slice(&vitals.health.to_be_bytes());
    write_varint(&mut output, i32::from(vitals.food));
    output.extend_from_slice(&vitals.saturation.to_be_bytes());
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
    fn encodes_vanilla_health_food_and_saturation() {
        assert_eq!(
            encode_set_health(Vitals::default()).unwrap(),
            vec![0x41, 0xa0, 0, 0, 0x14, 0x40, 0xa0, 0, 0]
        );
    }

    #[test]
    fn validates_finite_non_negative_floats() {
        let vitals = Vitals {
            health: f32::NAN,
            ..Vitals::default()
        };
        assert!(matches!(
            encode_set_health(vitals),
            Err(HealthEncodeError::InvalidHealth { .. })
        ));
        let vitals = Vitals {
            saturation: -1.0,
            ..Vitals::default()
        };
        assert_eq!(
            encode_set_health(vitals),
            Err(HealthEncodeError::InvalidSaturation { value: -1.0 })
        );
    }
}
