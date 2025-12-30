use anchor_lang::prelude::*;

use crate::{
    constants::{SECONDS_PER_DAY, SECONDS_PER_HOUR},
    errors::OndoError,
};

/// GM Token Manager State account - tracks global state for GM Token operations
#[account]
#[derive(InitSpace)]
pub struct GMTokenManagerState {
    // Monotonically increasing execution ID for mint/redeem operations
    pub execution_id: Option<u128>,

    // True if the token factory is paused - prevents new GM Tokens from being created
    pub factory_paused: bool,

    // True if redemption is paused
    pub redemption_paused: bool,

    // True if minting is paused
    pub minting_paused: bool,

    // Bump used to derive the PDA for this account
    // Stored so we don't need to recalculate it later
    pub bump: u8,

    /// Ethereum address (20 bytes) for secp256k1 signature verification
    /// Used to verify attestation signatures for buy/sell operations
    /// All zeros ([0u8; 20]) means not set
    pub attestation_signer_secp: [u8; 20],

    /// Trading hours offset from UTC in seconds
    /// Positive values are east of UTC, negative values are west of UTC
    pub trading_hours_offset: i64,
}

impl GMTokenManagerState {
    pub fn next_execution_id(&mut self) -> Result<u128> {
        let current_id = self.execution_id.unwrap_or(0);
        let next_id = current_id.checked_add(1).ok_or(OndoError::MathOverflow)?;
        self.execution_id = Some(next_id);
        Ok(next_id)
    }

    // Validate the trading hours offset
    // Check if the trading hours offset is within the allowed range
    // -12 hours to +14 hours in seconds
    pub fn validate_trading_hours_offset(&self, trading_hours_offset: i64) -> Result<()> {
        if !(-12 * SECONDS_PER_HOUR..=14 * SECONDS_PER_HOUR).contains(&trading_hours_offset) {
            return err!(OndoError::MaximumOffsetExceeded);
        }

        Ok(())
    }

    pub fn check_is_valid_hours(&self, timestamp: i64) -> Result<()> {
        let adjusted_timestamp = timestamp + self.trading_hours_offset;

        let days_since_epoch = adjusted_timestamp / SECONDS_PER_DAY;

        // +3 shifts Thursday to become Monday (0)
        let day_of_week = (days_since_epoch + 3).rem_euclid(7);

        // 5 = Saturday, 6 = Sunday
        require!(day_of_week < 5, OndoError::OutsideMarketHours);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state(trading_hours_offset: i64) -> GMTokenManagerState {
        GMTokenManagerState {
            execution_id: None,
            factory_paused: false,
            redemption_paused: false,
            minting_paused: false,
            bump: 0,
            attestation_signer_secp: [0u8; 20],
            trading_hours_offset,
        }
    }

    #[test]
    fn test_validate_trading_hours_offset_valid_boundaries() {
        let state = create_test_state(0);

        // Test minimum valid offset (-12 hours)
        assert!(state
            .validate_trading_hours_offset(-12 * SECONDS_PER_HOUR)
            .is_ok());

        // Test maximum valid offset (+14 hours)
        assert!(state
            .validate_trading_hours_offset(14 * SECONDS_PER_HOUR)
            .is_ok());
    }

    #[test]
    fn test_validate_trading_hours_offset_valid_common_timezones() {
        let state = create_test_state(0);

        // UTC (0)
        assert!(state.validate_trading_hours_offset(0).is_ok());

        // EST (-5 hours)
        assert!(state
            .validate_trading_hours_offset(-5 * SECONDS_PER_HOUR)
            .is_ok());

        // PST (-8 hours)
        assert!(state
            .validate_trading_hours_offset(-8 * SECONDS_PER_HOUR)
            .is_ok());

        // JST (+9 hours)
        assert!(state
            .validate_trading_hours_offset(9 * SECONDS_PER_HOUR)
            .is_ok());

        // AEST (+10 hours)
        assert!(state
            .validate_trading_hours_offset(10 * SECONDS_PER_HOUR)
            .is_ok());
    }

    #[test]
    fn test_validate_trading_hours_offset_invalid_too_negative() {
        let state = create_test_state(0);

        // -13 hours (just beyond minimum)
        let result = state.validate_trading_hours_offset(-13 * SECONDS_PER_HOUR);
        assert!(result.is_err());

        // -100 hours (way beyond minimum)
        let result = state.validate_trading_hours_offset(-100 * SECONDS_PER_HOUR);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_trading_hours_offset_invalid_too_positive() {
        let state = create_test_state(0);

        // +15 hours (just beyond maximum)
        let result = state.validate_trading_hours_offset(15 * SECONDS_PER_HOUR);
        assert!(result.is_err());

        // +100 hours (way beyond maximum)
        let result = state.validate_trading_hours_offset(100 * SECONDS_PER_HOUR);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_is_valid_hours_weekdays() {
        let state = create_test_state(0);

        // Unix epoch (Jan 1, 1970) was a Thursday
        // Let's test various weekdays

        // Thursday, Jan 1, 1970 00:00:00 UTC
        assert!(state.check_is_valid_hours(0).is_ok());

        // Friday, Jan 2, 1970 00:00:00 UTC
        assert!(state.check_is_valid_hours(SECONDS_PER_DAY).is_ok());

        // Monday, Jan 5, 1970 00:00:00 UTC
        assert!(state.check_is_valid_hours(4 * SECONDS_PER_DAY).is_ok());

        // Tuesday, Jan 6, 1970 00:00:00 UTC
        assert!(state.check_is_valid_hours(5 * SECONDS_PER_DAY).is_ok());

        // Wednesday, Jan 7, 1970 00:00:00 UTC
        assert!(state.check_is_valid_hours(6 * SECONDS_PER_DAY).is_ok());
    }

    #[test]
    fn test_check_is_valid_hours_weekends() {
        let state = create_test_state(0);

        // Saturday, Jan 3, 1970 00:00:00 UTC
        let result = state.check_is_valid_hours(2 * SECONDS_PER_DAY);
        assert!(result.is_err());

        // Sunday, Jan 4, 1970 00:00:00 UTC
        let result = state.check_is_valid_hours(3 * SECONDS_PER_DAY);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_is_valid_hours_with_trading_hours_offset() {
        // Test with +8 hours trading_hours offset (e.g., Singapore/Hong Kong)
        let state = create_test_state(8 * SECONDS_PER_HOUR);

        // Friday, Jan 2, 1970 16:00:00 UTC
        // This is Saturday 00:00:00 in UTC+8, so it should fail
        let friday_utc_late = SECONDS_PER_DAY + (16 * SECONDS_PER_HOUR);
        let result = state.check_is_valid_hours(friday_utc_late);
        assert!(result.is_err());

        // Friday, Jan 2, 1970 12:00:00 UTC
        // This is Friday 20:00:00 in UTC+8, so it should pass
        let friday_utc_noon = SECONDS_PER_DAY + (12 * SECONDS_PER_HOUR);
        assert!(state.check_is_valid_hours(friday_utc_noon).is_ok());
    }

    #[test]
    fn test_check_is_valid_hours_with_negative_trading_hours_offset() {
        // Test with -5 hours trading_hours offset (e.g., EST)
        let state = create_test_state(-5 * SECONDS_PER_HOUR);

        // Monday, Jan 5, 1970 03:00:00 UTC
        // This is Sunday 22:00:00 in UTC-5, so it should fail
        let monday_utc_early = (4 * SECONDS_PER_DAY) + (3 * SECONDS_PER_HOUR);
        let result = state.check_is_valid_hours(monday_utc_early);
        assert!(result.is_err());

        // Monday, Jan 5, 1970 06:00:00 UTC
        // This is Monday 01:00:00 in UTC-5, so it should pass
        let monday_utc_morning = (4 * SECONDS_PER_DAY) + (6 * SECONDS_PER_HOUR);
        assert!(state.check_is_valid_hours(monday_utc_morning).is_ok());
    }

    #[test]
    fn test_eastern_standard_time_8pm_friday_sunday_closure() {
        // EST offset for 8PM Friday -> 8PM Sunday closure: -3600 seconds (UTC-1)
        // This is EST (UTC-5) + 4-hour alignment = UTC-1
        let state = create_test_state(-3600);

        // Friday, Jan 2, 1970 20:00:00 EST = Friday, Jan 3, 1970 01:00:00 UTC
        // With -3600 offset: 01:00:00 UTC - 1 hour = 00:00:00 UTC = midnight Saturday (invalid)
        let friday_8pm_est_utc = SECONDS_PER_DAY + (25 * SECONDS_PER_HOUR); // Jan 3 01:00 UTC
        let result = state.check_is_valid_hours(friday_8pm_est_utc);
        assert!(
            result.is_err(),
            "8PM Friday EST should be invalid (maps to Saturday)"
        );

        // Friday, Jan 2, 1970 19:59:59 EST = Friday, Jan 3, 1970 00:59:59 UTC
        // With -3600 offset: 00:59:59 UTC - 1 hour = 23:59:59 Friday (valid)
        let friday_7_59_59_pm_est_utc = SECONDS_PER_DAY + (24 * SECONDS_PER_HOUR) + (59 * 60) + 59;
        assert!(
            state
                .check_is_valid_hours(friday_7_59_59_pm_est_utc)
                .is_ok(),
            "7:59:59 PM Friday EST should be valid"
        );

        // Sunday, Jan 4, 1970 20:00:00 EST = Monday, Jan 5, 1970 01:00:00 UTC
        // With -3600 offset: 01:00:00 UTC - 1 hour = 00:00:00 Monday (valid)
        let sunday_8pm_est_utc = (4 * SECONDS_PER_DAY) + SECONDS_PER_HOUR; // Jan 5 01:00 UTC
        assert!(
            state.check_is_valid_hours(sunday_8pm_est_utc).is_ok(),
            "8PM Sunday EST should be valid (maps to Monday midnight)"
        );

        // Sunday, Jan 4, 1970 19:59:59 EST = Monday, Jan 5, 1970 00:59:59 UTC
        // With -3600 offset: 00:59:59 UTC - 1 hour = 23:59:59 Sunday (invalid)
        let sunday_7_59_59_pm_est_utc = (4 * SECONDS_PER_DAY) + (59 * 60) + 59;
        let result = state.check_is_valid_hours(sunday_7_59_59_pm_est_utc);
        assert!(
            result.is_err(),
            "7:59:59 PM Sunday EST should be invalid (still Sunday)"
        );
    }

    #[test]
    fn test_eastern_daylight_time_8pm_friday_sunday_closure() {
        // EDT offset for 8PM Friday -> 8PM Sunday closure: 0 seconds (UTC+0)
        // This is EDT (UTC-4) + 4-hour alignment = UTC+0
        let state = create_test_state(0);

        // Friday, Jan 2, 1970 20:00:00 EDT = Saturday, Jan 3, 1970 00:00:00 UTC
        // With 0 offset: 00:00:00 UTC = midnight Saturday (invalid)
        let friday_8pm_edt_utc = 2 * SECONDS_PER_DAY; // Jan 3 00:00 UTC
        let result = state.check_is_valid_hours(friday_8pm_edt_utc);
        assert!(
            result.is_err(),
            "8PM Friday EDT should be invalid (maps to Saturday)"
        );

        // Friday, Jan 2, 1970 19:59:59 EDT = Friday, Jan 2, 1970 23:59:59 UTC
        // With 0 offset: 23:59:59 UTC = still Friday (valid)
        let friday_7_59_59_pm_edt_utc = (2 * SECONDS_PER_DAY) - 1;
        assert!(
            state
                .check_is_valid_hours(friday_7_59_59_pm_edt_utc)
                .is_ok(),
            "7:59:59 PM Friday EDT should be valid"
        );

        // Sunday, Jan 4, 1970 20:00:00 EDT = Monday, Jan 5, 1970 00:00:00 UTC
        // With 0 offset: 00:00:00 UTC = midnight Monday (valid)
        let sunday_8pm_edt_utc = 4 * SECONDS_PER_DAY; // Jan 5 00:00 UTC
        assert!(
            state.check_is_valid_hours(sunday_8pm_edt_utc).is_ok(),
            "8PM Sunday EDT should be valid (maps to Monday midnight)"
        );

        // Sunday, Jan 4, 1970 19:59:59 EDT = Sunday, Jan 4, 1970 23:59:59 UTC
        // With 0 offset: 23:59:59 UTC = still Sunday (invalid)
        let sunday_7_59_59_pm_edt_utc = (4 * SECONDS_PER_DAY) - 1;
        let result = state.check_is_valid_hours(sunday_7_59_59_pm_edt_utc);
        assert!(
            result.is_err(),
            "7:59:59 PM Sunday EDT should be invalid (still Sunday)"
        );
    }

    #[test]
    fn test_validate_eastern_time_offsets() {
        let state = create_test_state(0);

        // EST offset (UTC-1): -3600 seconds
        assert!(
            state.validate_trading_hours_offset(-3600).is_ok(),
            "EST offset (-3600s) should be valid"
        );

        // EDT offset (UTC+0): 0 seconds
        assert!(
            state.validate_trading_hours_offset(0).is_ok(),
            "EDT offset (0s) should be valid"
        );
    }
}
