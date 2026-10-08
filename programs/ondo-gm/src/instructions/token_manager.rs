use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{
        burn_checked, mint_to, transfer_checked, BurnChecked, Mint, MintTo, TokenAccount,
        TokenInterface, TransferChecked,
    },
};

use crate::{
    constants::*,
    errors::OndoError,
    instructions::{
        create_attestation_account, usdc_oracle_sanity_check, verify_attestation, verify_whitelist,
    },
    state::{GMTokenManagerState, OndoUser, OracleSanityCheck, TokenLimit, USDonManagerState},
    utils::{mul_div, normalize_decimals},
};

pub struct TokenManager<'a, 'info> {
    pub user: &'a mut Signer<'info>,
    pub mint: &'a mut InterfaceAccount<'info, Mint>,
    pub mint_authority: &'a UncheckedAccount<'info>,
    pub ondo_user: &'a mut Account<'info, OndoUser>,
    pub token_limit_account: &'a mut Account<'info, TokenLimit>,
    pub sanity_check_account: &'a mut Account<'info, OracleSanityCheck>,
    pub user_token_account: &'a mut InterfaceAccount<'info, TokenAccount>,
    pub attestation_id_account: &'a mut UncheckedAccount<'info>,
    pub whitelist: &'a UncheckedAccount<'info>,
    pub token_program: &'a Interface<'info, TokenInterface>,
    pub spl_token_program: Option<&'a Program<'info, Token>>,
    pub usdc_price_update: Option<&'a UncheckedAccount<'info>>,
    pub usdc_vault: Option<&'a mut InterfaceAccount<'info, TokenAccount>>,
    pub usdon_vault: &'a mut InterfaceAccount<'info, TokenAccount>,
    pub usdc_mint: Option<&'a InterfaceAccount<'info, Mint>>,
    pub user_usdc_token_account: Option<&'a mut InterfaceAccount<'info, TokenAccount>>,
    pub usdon_mint: &'a InterfaceAccount<'info, Mint>,
    pub user_usdon_token_account: &'a mut InterfaceAccount<'info, TokenAccount>,
    pub usdon_manager_state: &'a Account<'info, USDonManagerState>,
    pub gmtoken_manager_state: &'a mut Account<'info, GMTokenManagerState>,
    pub instructions: &'a UncheckedAccount<'info>,
}

impl<'a, 'info> TokenManager<'a, 'info> {
    pub fn validate(&self, is_usdon: bool) -> Result<()> {
        // Validate the user's USDon token account
        require_keys_eq!(
            self.user_usdon_token_account.mint,
            self.usdon_mint.key(),
            OndoError::InvalidTokenAccount
        );

        require_keys_eq!(
            self.user_usdon_token_account.owner,
            self.user.key(),
            OndoError::InvalidTokenAccount
        );

        require_keys_eq!(
            *self.user_usdon_token_account.to_account_info().owner,
            self.token_program.key(),
            OndoError::InvalidTokenAccount
        );

        if !is_usdon {
            let spl_token = self
                .spl_token_program
                .ok_or(OndoError::TokenProgramNotProvided)?;

            let usdc_mint = self.usdc_mint.ok_or(OndoError::MintNotProvided)?;

            // SAFETY: is_usdon is false, so user_usdc_token_account must be Some
            let user_usdc_token_account = self.user_usdc_token_account.as_ref().unwrap();

            // Validate the user's USDC token account
            require_keys_eq!(
                user_usdc_token_account.mint,
                usdc_mint.key(),
                OndoError::InvalidTokenAccount
            );

            require_keys_eq!(
                user_usdc_token_account.owner,
                self.user.key(),
                OndoError::InvalidTokenAccount
            );

            require_keys_eq!(
                *user_usdc_token_account.to_account_info().owner,
                spl_token.key(),
                OndoError::InvalidTokenAccount
            );
        }

        Ok(())
    }

    /// Swaps USDC tokens for USDon tokens (1:1 exchange).
    ///
    /// This method handles the conversion of USDC to USDon tokens with the following steps:
    /// 1. Validates the amount and, if enabled, applies the USDC oracle guard without repricing
    /// 2. Transfers USDC from user to protocol vault
    /// 3. Returns the calculated USDon amount to be burned
    ///
    /// # Arguments
    /// * `amount_in` - The amount of USDC tokens to swap (must be > 0)
    ///
    /// # Returns
    /// * `Result<u64>` - The amount of USDon tokens to be burned
    pub fn swap_usdc_to_usdon(&mut self, amount_in: u64) -> Result<u64> {
        // Validate that input amount is greater than zero
        require_gt!(amount_in, 0);

        // Perform sanity checks on the USDC token
        if self.usdon_manager_state.oracle_price_enabled {
            usdc_oracle_sanity_check(
                self.usdc_price_update
                    .ok_or(OndoError::USDCOracleNotProvided)?,
                self.usdon_manager_state,
            )?;
        }

        let usdc_mint = self.usdc_mint.as_ref().ok_or(OndoError::InvalidInputMint)?;

        // Transfer USDC tokens from user to protocol vault
        // This locks the user's USDC in the protocol's vault
        transfer_checked(
            CpiContext::new(
                self.spl_token_program
                    .as_ref()
                    .ok_or(OndoError::TokenProgramNotProvided)?
                    .to_account_info(),
                TransferChecked {
                    from: self
                        .user_usdc_token_account
                        .as_ref()
                        .ok_or(OndoError::InvalidTokenAccount)?
                        .to_account_info(),
                    mint: usdc_mint.to_account_info(),
                    to: self
                        .usdc_vault
                        .as_ref()
                        .ok_or(OndoError::InvalidTokenAccount)?
                        .to_account_info(),
                    authority: self.user.to_account_info(),
                },
            ),
            amount_in,
            usdc_mint.decimals,
        )?;

        // Normalize decimals from USDC (6 decimals) to USDon (9 decimals)
        let normalized_amount_out = normalize_decimals(
            amount_in,
            usdc_mint.decimals,
            self.usdon_mint.decimals,
            false,
        )?;

        // Return the USDon amount to burn from the vault in the calling instruction
        Ok(normalized_amount_out)
    }

    /// Swaps USDon tokens for USDC tokens (1:1 exchange).
    ///
    /// This method handles the conversion of USDon to USDC tokens with the following steps:
    /// 1. Validates the amount and, if enabled, applies the USDC oracle guard without repricing
    /// 2. Transfers USDon from user to protocol vault
    /// 3. Transfers USDC from protocol vault to user
    ///
    /// # Arguments
    /// * `amount_in` - The amount of USDon tokens to swap (must be > 0)
    ///
    /// # Returns
    /// * `Result<()>` - Success if swap completes without errors
    pub fn swap_usdon_to_usdc(&mut self, amount_in: u64) -> Result<()> {
        // Validate that input amount is greater than zero
        require_gt!(amount_in, 0);

        // Perform sanity checks on the USDC token
        if self.usdon_manager_state.oracle_price_enabled {
            usdc_oracle_sanity_check(
                self.usdc_price_update
                    .ok_or(OndoError::USDCOracleNotProvided)?,
                self.usdon_manager_state,
            )?;
        }

        let usdc_mint = self.usdc_mint.as_ref().ok_or(OndoError::InvalidInputMint)?;

        // Normalize decimals from USDon (9 decimals) to USDC (6 decimals)
        let normalized_amount_out = normalize_decimals(
            amount_in,
            self.usdon_mint.decimals,
            usdc_mint.decimals,
            false,
        )?;

        require!(normalized_amount_out > 0, OndoError::InvalidAmount);

        // Ensure we transfer the correct amount of USDon tokens
        let usdon_amount_to_transfer = normalize_decimals(
            normalized_amount_out,
            usdc_mint.decimals,
            self.usdon_mint.decimals,
            false,
        )?;

        // Step 1: Transfer USDon tokens from user to protocol vault
        // This reduces the user's USDon balance and increases the protocol's USDon vault
        transfer_checked(
            CpiContext::new(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: self.user_usdon_token_account.to_account_info(),
                    mint: self.usdon_mint.to_account_info(),
                    to: self.usdon_vault.to_account_info(),
                    authority: self.user.to_account_info(),
                },
            ),
            usdon_amount_to_transfer,
            self.usdon_mint.decimals,
        )?;

        // Step 2: Transfer USDC tokens from protocol vault to user
        // This releases USDC from the protocol's vault to the user's account
        if normalized_amount_out != 0 {
            transfer_checked(
                CpiContext::new_with_signer(
                    self.spl_token_program
                        .as_ref()
                        .ok_or(OndoError::TokenProgramNotProvided)?
                        .to_account_info(),
                    TransferChecked {
                        from: self
                            .usdc_vault
                            .as_ref()
                            .ok_or(OndoError::InvalidTokenAccount)?
                            .to_account_info(),
                        mint: usdc_mint.to_account_info(),
                        to: self
                            .user_usdc_token_account
                            .as_ref()
                            .ok_or(OndoError::InvalidTokenAccount)?
                            .to_account_info(),
                        authority: self.usdon_manager_state.to_account_info(),
                    },
                    &[&[USDON_MANAGER_STATE_SEED, &[self.usdon_manager_state.bump]]],
                ),
                normalized_amount_out,
                usdc_mint.decimals,
            )?;
        }

        Ok(())
    }
}

/// Mints GM Tokens to the user's token account after verifying the attestation.
/// USDon payments transfer to the vault; USDC payments transfer to the USDC vault
/// and burn the corresponding USDon from the USDon vault.
/// # Arguments
/// * `ctx` - The TokenManager context containing all necessary accounts.
/// * `attestation_id` - A unique 16-byte identifier for the attestation.
/// * `price` - The price associated with the attestation.
/// * `amount` - The amount of GM Tokens to mint.
/// * `expiration` - The expiration timestamp of the attestation.
/// * `is_usdon` - A boolean indicating if the user is paying with USDon (true) or USDC (false).
/// * `ondo_user_bump` - The bump seed for the Ondo user account PDA.
/// * `attestation_id_account_bump` - The bump seed for the attestation ID account PDA.
/// * `mint_authority_bump` - The bump seed for the mint authority PDA.
/// # Returns
/// * `Result<()>` - Ok if the minting process is successful, Err otherwise.
#[allow(clippy::too_many_arguments)]
pub fn mint_with_attestation(
    ctx: &mut TokenManager,
    attestation_id: [u8; 16],
    price: u64,
    amount: u64,
    expiration: i64,
    is_usdon: bool,
    ondo_user_bump: u8,
    attestation_id_account_bump: u8,
    mint_authority_bump: u8,
) -> Result<()> {
    // Validate token accounts
    ctx.validate(is_usdon)?;

    // Check if minting is paused
    require!(
        !ctx.gmtoken_manager_state.minting_paused,
        OndoError::TokenMintingPaused
    );

    // Check if token-level minting is paused
    require!(
        !ctx.token_limit_account.minting_paused,
        OndoError::TokenMintingPaused
    );

    // Verify user is whitelisted
    verify_whitelist(&ctx.whitelist.try_borrow_data()?)?;

    // Validate input parameters
    require_gt!(amount, 0, OndoError::InvalidAmount);
    require_gt!(price, 0, OndoError::InvalidPrice);

    let current_timestamp = Clock::get()?.unix_timestamp;

    // Check attestation expiration
    require!(
        current_timestamp < expiration,
        OndoError::AttestationExpired
    );
    // on-chain double check that expiration is within allowed max duration
    require!(
        expiration - current_timestamp <= MAX_ATTESTATION_EXPIRATION,
        OndoError::AttestationExpirationTooLarge
    );

    // Create ondo user account if it doesn't exist
    ctx.ondo_user.initialize(
        &ctx.user.key(),
        &ctx.mint.key(),
        ctx.token_limit_account.default_user_rate_limit,
        ctx.token_limit_account.default_user_limit_window,
        ondo_user_bump,
    );

    // Create attestation account if it doesn't exist marking the attestation as consumed
    create_attestation_account(
        ctx.user,
        ctx.attestation_id_account,
        attestation_id,
        current_timestamp,
        attestation_id_account_bump,
    )?;

    // Verify the attestation signature
    verify_attestation(
        CHAIN_ID.to_bytes(),
        attestation_id,
        BUY,
        price,
        amount,
        expiration,
        ctx.user.key(),
        ctx.mint.key(),
        ctx.instructions,
        ctx.gmtoken_manager_state.attestation_signer_secp,
    )?;

    // Perform sanity check
    ctx.sanity_check_account
        .sanity_check(price, current_timestamp)?;

    // Check rate limit of the GM Token and user
    let amount_sent = mul_div(price, amount, PRICE_SCALING_FACTOR as u64, true)?;
    ctx.token_limit_account
        .check_and_update_limit(amount_sent, current_timestamp, true)?;
    ctx.ondo_user
        .check_and_update_limit(amount_sent, current_timestamp, true)?;

    // Handle payment based on user's choice of USDon or USDC
    match is_usdon {
        true => {
            require_gt!(amount_sent, 0, OndoError::InvalidAmount);

            // Transfer USDon from user's token account to USDon vault
            transfer_checked(
                CpiContext::new(
                    ctx.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.user_usdon_token_account.to_account_info(),
                        mint: ctx.usdon_mint.to_account_info(),
                        to: ctx.usdon_vault.to_account_info(),
                        authority: ctx.user.to_account_info(),
                    },
                ),
                amount_sent,
                ctx.usdon_mint.decimals,
            )?;
        }
        false => {
            let usdc_mint_decimals = ctx
                .usdc_mint
                .as_ref()
                .ok_or(OndoError::InvalidInputMint)?
                .decimals;

            // Normalize amount from GM Token decimals to USDC decimals
            let normalized_amount =
                normalize_decimals(amount_sent, ctx.mint.decimals, usdc_mint_decimals, true)?;

            // If the user wants to pay in USDC, transfer USDC from user to USDC vault
            let amount_to_burn = ctx.swap_usdc_to_usdon(normalized_amount)?;

            // Then burn USDon from the USDon vault
            burn_checked(
                CpiContext::new_with_signer(
                    ctx.token_program.to_account_info(),
                    BurnChecked {
                        mint: ctx.usdon_mint.to_account_info(),
                        from: ctx.usdon_vault.to_account_info(),
                        authority: ctx.mint_authority.to_account_info(),
                    },
                    &[&[MINT_AUTHORITY_SEED, &[mint_authority_bump]]],
                ),
                amount_to_burn,
                ctx.usdon_mint.decimals,
            )?;
        }
    }

    // Mint GM Tokens to the user's token account
    mint_to(
        CpiContext::new_with_signer(
            ctx.token_program.to_account_info(),
            MintTo {
                mint: ctx.mint.to_account_info(),
                to: ctx.user_token_account.to_account_info(),
                authority: ctx.mint_authority.to_account_info(),
            },
            &[&[MINT_AUTHORITY_SEED, &[mint_authority_bump]]],
        ),
        amount,
    )
}

/// Redeems GM Tokens from the user's token account after verifying the attestation.
/// Mints USDon or transfers USDC based on the user's payment choice.
/// # Arguments
/// * `ctx` - The TokenManager context containing all necessary accounts.
/// * `attestation_id` - A unique 16-byte identifier for the attestation.
/// * `price` - The price associated with the attestation.
/// * `amount` - The amount of GM Tokens to redeem.
/// * `expiration` - The expiration timestamp of the attestation.
/// * `is_usdon` - A boolean indicating if the user wants to receive USDon (true) or USDC (false).
/// * `ondo_user_bump` - The bump seed for the Ondo user account PDA.
/// * `attestation_id_account_bump` - The bump seed for the attestation ID account PDA.
/// * `mint_authority_bump` - The bump seed for the mint authority PDA.
/// # Returns
/// * `Result<()>` - Ok if the redemption process is successful, Err otherwise.
#[allow(clippy::too_many_arguments)]
pub fn redeem_with_attestation(
    ctx: &mut TokenManager,
    attestation_id: [u8; 16],
    price: u64,
    amount: u64,
    expiration: i64,
    is_usdon: bool,
    ondo_user_bump: u8,
    attestation_id_account_bump: u8,
    mint_authority_bump: u8,
) -> Result<()> {
    // Validate token accounts
    ctx.validate(is_usdon)?;

    // Check if redemptions are paused
    require!(
        !ctx.gmtoken_manager_state.redemption_paused,
        OndoError::TokenRedemptionPaused
    );

    // Check if token-level redemptions are paused
    require!(
        !ctx.token_limit_account.redemption_paused,
        OndoError::TokenRedemptionPaused
    );

    // Verify user is whitelisted
    verify_whitelist(&ctx.whitelist.try_borrow_data()?)?;

    // Validate input parameters
    require_gt!(amount, 0, OndoError::InvalidAmount);
    require_gt!(price, 0, OndoError::InvalidPrice);

    let current_timestamp = Clock::get()?.unix_timestamp;

    // Check attestation expiration
    require!(
        current_timestamp < expiration,
        OndoError::AttestationExpired
    );

    // on-chain double check that expiration is within allowed max duration
    require!(
        expiration - current_timestamp <= MAX_ATTESTATION_EXPIRATION,
        OndoError::AttestationExpirationTooLarge
    );

    // Create ondo user account if it doesn't exist
    ctx.ondo_user.initialize(
        &ctx.user.key(),
        &ctx.mint.key(),
        ctx.token_limit_account.default_user_rate_limit,
        ctx.token_limit_account.default_user_limit_window,
        ondo_user_bump,
    );

    // Create attestation account if it doesn't exist marking the attestation as consumed
    create_attestation_account(
        ctx.user,
        ctx.attestation_id_account,
        attestation_id,
        current_timestamp,
        attestation_id_account_bump,
    )?;

    // Verify the attestation signature
    verify_attestation(
        CHAIN_ID.to_bytes(),
        attestation_id,
        SELL,
        price,
        amount,
        expiration,
        ctx.user.key(),
        ctx.mint.key(),
        ctx.instructions,
        ctx.gmtoken_manager_state.attestation_signer_secp,
    )?;

    // Perform sanity check
    ctx.sanity_check_account
        .sanity_check(price, current_timestamp)?;

    // Check rate limit of the GM Token and user
    let rate_limited_amount = mul_div(price, amount, PRICE_SCALING_FACTOR as u64, true)?;
    ctx.token_limit_account.check_and_update_limit(
        rate_limited_amount,
        current_timestamp,
        false,
    )?;
    ctx.ondo_user
        .check_and_update_limit(rate_limited_amount, current_timestamp, false)?;

    // Round down: Protocol pays - protects the protocol
    let mint_amount = mul_div(price, amount, PRICE_SCALING_FACTOR as u64, false)?;

    require_gt!(mint_amount, 0, OndoError::InvalidAmount);

    let seeds = &[MINT_AUTHORITY_SEED, &[mint_authority_bump]];
    let signer_seeds = &[&seeds[..]];

    // Mint USDon to user's token account
    mint_to(
        CpiContext::new_with_signer(
            ctx.token_program.to_account_info(),
            MintTo {
                mint: ctx.usdon_mint.to_account_info(),
                to: ctx.user_usdon_token_account.to_account_info(),
                authority: ctx.mint_authority.to_account_info(),
            },
            signer_seeds,
        ),
        mint_amount,
    )?;

    if !is_usdon {
        // If the user wants to be paid in USDC, transfer USDon from user to the USDon vault
        // Then transfer USDC from the USDC vault to the user
        ctx.swap_usdon_to_usdc(mint_amount)?;
    }

    // Burn GM tokens from the user's token account
    burn_checked(
        CpiContext::new(
            ctx.token_program.to_account_info(),
            BurnChecked {
                mint: ctx.mint.to_account_info(),
                from: ctx.user_token_account.to_account_info(),
                authority: ctx.user.to_account_info(),
            },
        ),
        amount,
        ctx.mint.decimals,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;

    // Isolates the deviation band: checking at the reference's own update time
    // means zero elapsed time, so freshness always passes.
    fn validate_price_deviation(sanity_check: &OracleSanityCheck, price: u64) -> Result<()> {
        sanity_check.sanity_check(price, sanity_check.price_last_updated)
    }

    #[test]
    fn test_price_update_v2_deserialization() {
        // Retreived using `solana account <account_address>` on a real Pyth price account
        let account_data: Vec<u8> = vec![
            0x22, 0xf1, 0x23, 0x63, 0x9d, 0x7e, 0xf4, 0xcd, 0xbe, 0x93, 0x9a, 0x83, 0x09, 0xf5,
            0x64, 0x07, 0x18, 0x7f, 0xff, 0x30, 0xac, 0x54, 0xb1, 0x69, 0x49, 0x8b, 0xe9, 0x9f,
            0x6d, 0x8e, 0x1b, 0xfd, 0x42, 0x44, 0x68, 0x0c, 0xd4, 0xf7, 0xd1, 0xe2, 0x01, 0xea,
            0xa0, 0x20, 0xc6, 0x1c, 0xc4, 0x79, 0x71, 0x28, 0x13, 0x46, 0x1c, 0xe1, 0x53, 0x89,
            0x4a, 0x96, 0xa6, 0xc0, 0x0b, 0x21, 0xed, 0x0c, 0xfc, 0x27, 0x98, 0xd1, 0xf9, 0xa9,
            0xe9, 0xc9, 0x4a, 0x62, 0x77, 0xf5, 0x05, 0x00, 0x00, 0x00, 0x00, 0xf6, 0x7e, 0x01,
            0x00, 0x00, 0x00, 0x00, 0x00, 0xf8, 0xff, 0xff, 0xff, 0x90, 0x39, 0x1f, 0x69, 0x00,
            0x00, 0x00, 0x00, 0x8f, 0x39, 0x1f, 0x69, 0x00, 0x00, 0x00, 0x00, 0x5e, 0x7c, 0xf5,
            0x05, 0x00, 0x00, 0x00, 0x00, 0x7c, 0x77, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18,
            0x7c, 0x34, 0x19, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let result = PriceUpdateV2::try_deserialize(&mut &account_data[..]);
        assert!(
            result.is_ok(),
            "Deserialization with full data should succeed. Error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_sanity_check_price_within_range() {
        let sanity_check = OracleSanityCheck {
            mint: Pubkey::default(),
            last_price: 1_000_000_000,           // $1.00
            allowed_deviation_bps: 500,          // 5% (500 basis points)
            max_time_delay: 7 * SECONDS_PER_DAY, // 7 days in seconds
            price_last_updated: 0,
            bump: 0,
        };

        // Test price at the center (should pass)
        let result = validate_price_deviation(&sanity_check, 1_000_000_000);
        assert!(result.is_ok());

        // Test price at upper bound (should pass)
        // 5% of 1_000_000_000 = 50_000_000
        // max_price = 1_050_000_000
        let result = validate_price_deviation(&sanity_check, 1_050_000_000);
        assert!(result.is_ok());

        // Test price at lower bound (should pass)
        // min_price = 950_000_000
        let result = validate_price_deviation(&sanity_check, 950_000_000);
        assert!(result.is_ok());

        // Test price slightly above upper bound (should fail)
        let result = validate_price_deviation(&sanity_check, 1_050_000_001);
        assert!(result.is_err());
        assert_eq!(result, Err(OndoError::PriceExceedsMaxDeviation.into()));

        // Test price slightly below lower bound (should fail)
        let result = validate_price_deviation(&sanity_check, 949_999_999);
        assert!(result.is_err());
        assert_eq!(result, Err(OndoError::PriceBelowMinDeviation.into()));
    }

    #[test]
    fn test_sanity_check_time_delay() {
        let base_timestamp = 1_000_000_000i64;

        let sanity_check = OracleSanityCheck {
            mint: Pubkey::default(),
            last_price: 1_000_000_000,
            allowed_deviation_bps: 500,
            max_time_delay: 7 * SECONDS_PER_DAY, // 7 days in seconds
            price_last_updated: base_timestamp,
            bump: 0,
        };

        // Test within time limit (should pass)
        // 6 days = 6 * 86400 = 518400 seconds
        let current_timestamp = base_timestamp + (6 * SECONDS_PER_DAY);
        let result = sanity_check.validate_freshness(current_timestamp);
        assert!(result.is_ok());

        // Test exactly at time limit (should pass)
        // 7 days = 7 * 86400 = 604800 seconds
        let current_timestamp = base_timestamp + (7 * SECONDS_PER_DAY);
        let result = sanity_check.validate_freshness(current_timestamp);
        assert!(result.is_ok());

        // Test beyond time limit (should fail)
        // 8 days = 8 * 86400 = 691200 seconds
        let current_timestamp = base_timestamp + (8 * SECONDS_PER_DAY);
        let result = sanity_check.validate_freshness(current_timestamp);
        assert!(result.is_err());
        assert_eq!(result, Err(OndoError::MaxTimeDelayExceeded.into()));
    }

    #[test]
    fn test_sanity_check_zero_deviation() {
        let sanity_check = OracleSanityCheck {
            mint: Pubkey::default(),
            last_price: 1_000_000_000,
            allowed_deviation_bps: 0, // 0% deviation
            max_time_delay: 7 * SECONDS_PER_DAY,
            price_last_updated: 0,
            bump: 0,
        };

        // Only exact price should pass
        let result = validate_price_deviation(&sanity_check, 1_000_000_000);
        assert!(result.is_ok());

        // Any other price should fail
        let result = validate_price_deviation(&sanity_check, 1_000_000_001);
        assert!(result.is_err());

        let result = validate_price_deviation(&sanity_check, 999_999_999);
        assert!(result.is_err());
    }

    #[test]
    fn test_sanity_check_large_deviation() {
        let sanity_check = OracleSanityCheck {
            mint: Pubkey::default(),
            last_price: 1_000_000_000,
            allowed_deviation_bps: 5000, // 50% deviation
            max_time_delay: 7 * SECONDS_PER_DAY,
            price_last_updated: 0,
            bump: 0,
        };

        // Test wide range should pass
        // 50% of 1_000_000_000 = 500_000_000
        // Range: 500_000_000 to 1_500_000_000

        let result = validate_price_deviation(&sanity_check, 500_000_000);
        assert!(result.is_ok());

        let result = validate_price_deviation(&sanity_check, 1_500_000_000);
        assert!(result.is_ok());

        let result = validate_price_deviation(&sanity_check, 750_000_000);
        assert!(result.is_ok());

        let result = validate_price_deviation(&sanity_check, 1_250_000_000);
        assert!(result.is_ok());

        // Outside range should fail
        let result = validate_price_deviation(&sanity_check, 499_999_999);
        assert!(result.is_err());

        let result = validate_price_deviation(&sanity_check, 1_500_000_001);
        assert!(result.is_err());
    }

    #[test]
    fn test_sanity_check_combined_validation() {
        let base_timestamp = 1_000_000_000i64;

        let sanity_check = OracleSanityCheck {
            mint: Pubkey::default(),
            last_price: 1_000_000_000,
            allowed_deviation_bps: 500, // 5%
            max_time_delay: 7 * SECONDS_PER_DAY,
            price_last_updated: base_timestamp,
            bump: 0,
        };

        // Valid price and time (should pass)
        let current_timestamp = base_timestamp + (5 * SECONDS_PER_DAY);
        let result = sanity_check.sanity_check(1_020_000_000, current_timestamp);
        assert!(result.is_ok());

        // Invalid price but valid time (should fail on price)
        let result = sanity_check.sanity_check(1_100_000_000, current_timestamp);
        assert!(result.is_err());
        assert_eq!(result, Err(OndoError::PriceExceedsMaxDeviation.into()));

        // Valid price but invalid time (should fail on time)
        let current_timestamp = base_timestamp + (10 * SECONDS_PER_DAY);
        let result = sanity_check.sanity_check(1_020_000_000, current_timestamp);
        assert!(result.is_err());
        assert_eq!(result, Err(OndoError::MaxTimeDelayExceeded.into()));
    }

    #[test]
    fn test_sanity_check_edge_cases() {
        // Test with very small price
        let sanity_check = OracleSanityCheck {
            mint: Pubkey::default(),
            last_price: 1_000,
            allowed_deviation_bps: 1000, // 10%
            max_time_delay: 7 * SECONDS_PER_DAY,
            price_last_updated: 0,
            bump: 0,
        };

        let result = validate_price_deviation(&sanity_check, 1_100);
        assert!(result.is_ok());

        let result = validate_price_deviation(&sanity_check, 900);
        assert!(result.is_ok());

        // Test with maximum deviation (100%)
        let sanity_check = OracleSanityCheck {
            mint: Pubkey::default(),
            last_price: 1_000_000_000,
            allowed_deviation_bps: 10_000, // 100%
            max_time_delay: 7 * SECONDS_PER_DAY,
            price_last_updated: 0,
            bump: 0,
        };

        let result = validate_price_deviation(&sanity_check, 0);
        assert!(result.is_ok());

        let result = validate_price_deviation(&sanity_check, 2_000_000_000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sanity_check_time_edge_cases() {
        let base_timestamp = 1_000_000_000i64;

        // Zero max_time_delay - only same instant should pass
        let sanity_check = OracleSanityCheck {
            mint: Pubkey::default(),
            last_price: 1_000_000_000,
            allowed_deviation_bps: 500,
            max_time_delay: 0, // 0 seconds
            price_last_updated: base_timestamp,
            bump: 0,
        };

        let result = sanity_check.validate_freshness(base_timestamp);
        assert!(result.is_ok());

        // Even 1 second later should fail
        let result = sanity_check.validate_freshness(base_timestamp + 1);
        assert!(result.is_err());

        // Very large max_time_delay (10 years in seconds)
        let sanity_check = OracleSanityCheck {
            mint: Pubkey::default(),
            last_price: 1_000_000_000,
            allowed_deviation_bps: 500,
            max_time_delay: 365 * 10 * SECONDS_PER_DAY, // 10 years in seconds
            price_last_updated: base_timestamp,
            bump: 0,
        };

        let current_timestamp = base_timestamp + (365 * 10 * SECONDS_PER_DAY);
        let result = sanity_check.validate_freshness(current_timestamp);
        assert!(result.is_ok());

        let current_timestamp = base_timestamp + (365 * 10 * SECONDS_PER_DAY) + 1;
        let result = sanity_check.validate_freshness(current_timestamp);
        assert!(result.is_err());
    }

    proptest! {
        #[test]
        fn test_sanity_check_price_fuzz(
            last_price in 100_000_000u64..=10_000_000_000u64,
            deviation_bps in 1u64..=10_000u64,
            price_change_bps in 0i64..=20_000i64,
        ) {
            let sanity_check = OracleSanityCheck {
                mint: Pubkey::default(),
                last_price,
                allowed_deviation_bps: deviation_bps,
                max_time_delay: 7 * SECONDS_PER_DAY,
                price_last_updated: 0,
                bump: 0,
            };

            // Calculate the actual price based on percentage change
            // price_change_bps ranges from 0 to 20000 (0% to 200%)
            // We want it to range from -100% to +100%, so subtract 10000
            let price_multiplier = BASIS_POINTS_DIVISOR as i64 + price_change_bps - 10_000;
            let new_price = ((last_price as i128 * price_multiplier as i128) / BASIS_POINTS_DIVISOR as i128).max(0) as u64;

            let result = validate_price_deviation(&sanity_check, new_price);

            // Calculate expected deviation
            let deviation = (last_price * deviation_bps) / BASIS_POINTS_DIVISOR;
            let max_price = last_price + deviation;
            let min_price = last_price.saturating_sub(deviation);

            if new_price > max_price {
                prop_assert!(result.is_err());
                prop_assert_eq!(result, Err(OndoError::PriceExceedsMaxDeviation.into()));
            } else if new_price < min_price {
                prop_assert!(result.is_err());
                prop_assert_eq!(result, Err(OndoError::PriceBelowMinDeviation.into()));
            } else {
                prop_assert!(result.is_ok());
            }
        }

        #[test]
        fn test_sanity_check_time_fuzz(
            max_time_delay in SECONDS_PER_DAY..=(365 * SECONDS_PER_DAY), // 1 day to 365 days in seconds
            seconds_elapsed in 0i64..=(730 * SECONDS_PER_DAY), // 0 to 730 days in seconds
        ) {
            let base_timestamp = 1_000_000_000i64;

            let sanity_check = OracleSanityCheck {
                mint: Pubkey::default(),
                last_price: 1_000_000_000,
                allowed_deviation_bps: 500,
                max_time_delay,
                price_last_updated: base_timestamp,
                bump: 0,
            };

            let current_timestamp = base_timestamp + seconds_elapsed;
            let result = sanity_check.validate_freshness(current_timestamp);

            if seconds_elapsed > max_time_delay {
                prop_assert!(result.is_err());
                prop_assert_eq!(result, Err(OndoError::MaxTimeDelayExceeded.into()));
            } else {
                prop_assert!(result.is_ok());
            }
        }
    }
}
