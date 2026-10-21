//! Community ratings use a finite 0–10 scale throughout storage and API responses.

pub(super) const MAXIMUM: f64 = 10.0;

/// Convert a provider's scale to Koko's scale, rejecting invalid values.
pub(super) fn normalize(
    value: f64,
    source_maximum: f64,
) -> Option<f32> {
    if !source_maximum.is_finite()
        || source_maximum <= 0.0
        || !value.is_finite()
        || !(0.0..=source_maximum).contains(&value)
    {
        return None;
    }
    Some((value / source_maximum * MAXIMUM) as f32)
}

/// Validate a stored rating without clamping bad data to the scale's endpoints.
pub(crate) fn validate(value: f32) -> Option<f32> {
    (value.is_finite() && (0.0..=MAXIMUM).contains(&f64::from(value))).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_scales_preserve_precision_and_endpoints() {
        assert_eq!(normalize(4.25, 5.0), Some(8.5));
        assert_eq!(normalize(8.25, 10.0), Some(8.25));
        assert_eq!(normalize(85.0, 100.0), Some(8.5));
        assert_eq!(normalize(0.0, 5.0), Some(0.0));
        assert_eq!(normalize(5.0, 5.0), Some(10.0));
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
        for value in [
            -0.1,
            10.01,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            assert_eq!(validate(value), None);
        }
        assert_eq!(validate(0.0), Some(0.0));
        assert_eq!(validate(10.0), Some(10.0));
    }
}
