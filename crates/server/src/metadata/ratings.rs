//! Community scores are stored as 0–100 integers and exposed on a 0–10 display scale.

pub(super) const DEFAULT_PROVIDER_MAXIMUM: f64 = 10.0;
const STORED_MAXIMUM: i16 = 100;

/// Round a valid provider average to the nearest integer on Koko's 0–100 storage scale.
pub(super) fn normalize(
    value: f64,
    source_maximum: f64,
) -> Option<i16> {
    if !source_maximum.is_finite()
        || source_maximum <= 0.0
        || !value.is_finite()
        || !(0.0..=source_maximum).contains(&value)
    {
        return None;
    }
    // Multiplying by the scale factor keeps 7.85 out of 10 at the 78.5 rounding boundary.
    let scaled = value * (f64::from(STORED_MAXIMUM) / source_maximum);
    let scaled = if scaled.is_finite() {
        scaled
    } else {
        value / source_maximum * f64::from(STORED_MAXIMUM)
    };
    Some(scaled.round() as i16)
}

/// Validate a stored score without clamping bad data to the scale's endpoints.
pub(crate) fn validate(value: i16) -> Option<i16> {
    (0..=STORED_MAXIMUM).contains(&value).then_some(value)
}

/// Convert storage units back to a 0–10 score for every public rating response.
pub(crate) fn display(value: i16) -> Option<f32> {
    validate(value).map(|value| f32::from(value) / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_scales_round_to_integer_scores_and_preserve_endpoints() {
        assert_eq!(normalize(4.25, 5.0), Some(85));
        assert_eq!(normalize(8.25, 10.0), Some(83));
        assert_eq!(normalize(8.24, 10.0), Some(82));
        assert_eq!(normalize(7.85, 10.0), Some(79));
        assert_eq!(normalize(3.925, 5.0), Some(79));
        assert_eq!(normalize(85.0, 100.0), Some(85));
        assert_eq!(normalize(0.0, 5.0), Some(0));
        assert_eq!(normalize(5.0, 5.0), Some(100));
        assert_eq!(normalize(f64::MIN_POSITIVE, f64::MIN_POSITIVE), Some(100));
        assert_eq!(normalize(f64::from_bits(1), f64::from_bits(1)), Some(100));
        assert_eq!(display(78), Some(7.8));
        assert_eq!(display(0), Some(0.0));
        assert_eq!(display(100), Some(10.0));
    }

    #[test]
    fn invalid_scores_and_scales_are_rejected_instead_of_clamped() {
        for value in [
            -1.0,
            5.01,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert_eq!(normalize(value, 5.0), None);
        }
        for scale in [
            0.0,
            -5.0,
            f64::NAN,
            f64::INFINITY,
        ] {
            assert_eq!(normalize(1.0, scale), None);
        }
        for value in [-1, 101, i16::MIN, i16::MAX] {
            assert_eq!(validate(value), None);
            assert_eq!(display(value), None);
        }
        assert_eq!(validate(0), Some(0));
        assert_eq!(validate(100), Some(100));
    }
}
