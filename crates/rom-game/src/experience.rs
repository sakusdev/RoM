//! Experience point and level conversion helpers.

#[must_use]
pub const fn points_to_next_level(level: u32) -> u32 {
    if level >= 30 {
        112 + (level - 30) * 9
    } else if level >= 15 {
        37 + (level - 15) * 5
    } else {
        7 + level * 2
    }
}

#[must_use]
pub const fn total_points_for_level(level: u32) -> u64 {
    let l = level as u128;
    let value = if level <= 16 {
        l * l + 6 * l
    } else if level <= 31 {
        // Reorder the polynomial so unsigned intermediate values never go
        // negative. The result is mathematically identical to
        // (5*l*l - 81*l + 720) / 2.
        (5 * l * l + 720 - 81 * l) / 2
    } else {
        // Level 32 is the important boundary: 9*l*l - 325*l is negative
        // before the +4440 term is applied, so the naïve unsigned expression
        // underflows even though the final polynomial is positive.
        (9 * l * l + 4440 - 325 * l) / 2
    };
    if value > u64::MAX as u128 {
        u64::MAX
    } else {
        value as u64
    }
}

#[must_use]
pub fn level_from_total(total: u64) -> u32 {
    let mut low = 0u32;
    let mut high = 1u32;
    while high < 1_000_000 && total_points_for_level(high) <= total {
        high = high.saturating_mul(2).min(1_000_000);
    }
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if total_points_for_level(mid) <= total {
            low = mid;
        } else {
            high = mid;
        }
    }
    if total_points_for_level(high) <= total {
        high
    } else {
        low
    }
}

#[must_use]
pub fn progress_from_total(total: u64) -> (u32, f32) {
    let level = level_from_total(total);
    let base = total_points_for_level(level);
    let needed = u64::from(points_to_next_level(level));
    let progress = (total.saturating_sub(base) as f64 / needed as f64).clamp(0.0, 1.0) as f32;
    (level, progress)
}

#[must_use]
pub fn add_points(total: u64, amount: u64) -> (u64, u32, f32) {
    let total = total.saturating_add(amount);
    let (level, progress) = progress_from_total(total);
    (total, level, progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_levels() {
        assert_eq!(points_to_next_level(0), 7);
        assert_eq!(total_points_for_level(1), 7);
        assert_eq!(total_points_for_level(16), 352);
    }

    #[test]
    fn inverse_level() {
        for level in [0, 1, 15, 16, 30, 31, 32, 100] {
            let total = total_points_for_level(level);
            assert_eq!(level_from_total(total), level);
        }
    }

    #[test]
    fn level_32_polynomial_does_not_underflow() {
        assert_eq!(total_points_for_level(32), 1628);
    }

    #[test]
    fn huge_levels_saturate_instead_of_overflowing() {
        assert_eq!(total_points_for_level(u32::MAX), u64::MAX);
    }

    #[test]
    fn adding_points_updates_progress() {
        let (total, level, progress) = add_points(0, 7);
        assert_eq!(total, 7);
        assert_eq!(level, 1);
        assert_eq!(progress, 0.0);
    }
}
