use anchor_lang::prelude::*;

use crate::{errors::OndoError, utils::mul_div};

/// Calculate the updated capacity used after applying decay based on time elapsed.
/// If the time since the last update exceeds the limit window, the capacity used is reset to zero.
/// Otherwise, the capacity used is reduced based on the rate limit and time elapsed.
/// # Arguments
/// * `time_since_last_update` - The time elapsed since the last update in seconds.
/// * `limit_window` - The time window for the rate limit in seconds.
/// * `capacity_used` - The current capacity used.
/// * `rate_limit` - The maximum rate limit allowed in the limit window.
/// # Returns
/// * `Result<u64>` - The updated capacity used after applying decay.
#[inline(always)]
pub fn calculate_capacity_used(
    time_since_last_update: i64,
    limit_window: u64,
    capacity_used: u64,
    rate_limit: u64,
) -> Result<u64> {
    require_gte!(
        time_since_last_update,
        0,
        OndoError::NegativeTimeSinceLastUpdate
    );

    let time_since_last_update_u64 = time_since_last_update as u64;
    if time_since_last_update_u64 >= limit_window {
        // Full capacity restored
        Ok(0)
    } else {
        // Validate limit_window is not zero to prevent division by zero
        // Round down: Restores less capacity used, making rate limiting more strict
        let decay = mul_div(rate_limit, time_since_last_update_u64, limit_window, false)?;

        if capacity_used > decay {
            capacity_used
                .checked_sub(decay)
                .ok_or(OndoError::MathOverflow.into())
        } else {
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_capacity_restored_when_time_exceeds_window() {
        // Time elapsed is greater than limit window
        let result = calculate_capacity_used(
            3600, // 1 hour
            1800, // 30 min window
            100,  // capacity used
            1000, // rate limit
        );
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_full_capacity_restored_when_time_equals_window() {
        // Time elapsed equals limit window
        let result = calculate_capacity_used(
            1800, // 30 min
            1800, // 30 min window
            100,  // capacity used
            1000, // rate limit
        );
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_partial_decay_basic() {
        // Half the window has passed, should decay half the rate limit
        let result = calculate_capacity_used(
            30,  // 30 seconds
            60,  // 60 second window
            100, // capacity used
            100, // rate limit
        );
        // Decay = (100/60) * 30 = 50
        // Capacity remaining = 100 - 50 = 50
        assert_eq!(result.unwrap(), 50);
    }

    #[test]
    fn test_decay_exceeds_capacity_used() {
        // Decay is larger than capacity used, should return 0
        let result = calculate_capacity_used(
            50,  // 50 seconds
            60,  // 60 second window
            10,  // capacity used
            120, // rate limit
        );
        // rate_per_second = 120/60 = 2
        // remainder = 120%60 = 0
        // decay_base = 2 * 50 = 100
        // decay_remainder = 0
        // total decay = 100
        // Since 10 < 100, result should be 0
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_no_time_passed() {
        // No time has passed, no decay
        let result = calculate_capacity_used(
            0,    // 0 seconds
            60,   // 60 second window
            100,  // capacity used
            1000, // rate limit
        );
        assert_eq!(result.unwrap(), 100);
    }

    #[test]
    fn test_with_remainder_in_division() {
        // Test when rate_limit doesn't divide evenly by limit_window
        let result = calculate_capacity_used(
            10,  // 10 seconds
            60,  // 60 second window
            100, // capacity used
            100, // rate limit
        );
        // rate_per_second = 100/60 = 1 (integer division)
        // remainder = 100%60 = 40
        // decay_base = 1 * 10 = 10
        // decay_remainder = (40 * 10) / 60 = 400/60 = 6
        // total decay = 10 + 6 = 16
        // result = 100 - 16 = 84
        assert_eq!(result.unwrap(), 84);
    }

    #[test]
    fn test_negative_time_since_last_update() {
        // Negative time should be converted and capped
        let result = calculate_capacity_used(
            -10,  // negative time
            60,   // 60 second window
            100,  // capacity used
            1000, // rate limit
        );
        // Should fail on conversion from negative i64 to u64
        assert!(result.is_err());
    }

    #[test]
    fn test_zero_limit_window_error() {
        // Zero limit window should return 0
        let result = calculate_capacity_used(
            10,   // 10 seconds
            0,    // 0 second window
            100,  // capacity used
            1000, // rate limit
        );
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_large_values_no_overflow() {
        // Test with large but safe values
        let result = calculate_capacity_used(100, 1000, u64::MAX / 2, u64::MAX / 4);
        assert!(result.is_ok());
    }

    #[test]
    fn test_exact_capacity_decay_match() {
        // When full window passes and time >= window, first condition returns 0
        let result = calculate_capacity_used(
            60,  // 60 seconds (equals window)
            60,  // 60 second window
            100, // capacity used
            100, // rate limit
        );
        // Since time_since_last_update (60) >= limit_window (60), returns 0
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_time_capping_at_limit_window() {
        // Time greater than window gets caught by first condition
        let result = calculate_capacity_used(
            200,  // time > window
            100,  // window
            1000, // capacity used
            500,  // rate limit
        );
        // Since time_since_last_update (200) >= limit_window (100), returns 0 immediately
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_zero_rate_limit() {
        // Zero rate limit means no capacity is restored
        let result = calculate_capacity_used(
            30,  // 30 seconds
            60,  // 60 second window
            100, // capacity used
            0,   // rate limit
        );
        // Decay = 0, so capacity remains unchanged
        assert_eq!(result.unwrap(), 100);
    }

    #[test]
    fn test_zero_capacity_used() {
        // Starting with zero capacity used should just return 0
        let result = calculate_capacity_used(
            30,  // 30 seconds
            60,  // 60 second window
            0,   // capacity used
            100, // rate limit
        );
        // No capacity was used, so nothing to decay from
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_very_small_time_increment() {
        // Very small time passed
        let result = calculate_capacity_used(
            1,    // 1 second
            3600, // 1 hour window
            1000, // capacity used
            3600, // rate limit (1 per second)
        );
        // Decay = 1 * 1 = 1
        // Result = 1000 - 1 = 999
        assert_eq!(result.unwrap(), 999);
    }
}
