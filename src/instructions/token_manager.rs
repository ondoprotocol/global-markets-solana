use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::Instruction,
    program::{invoke, invoke_signed},
    system_instruction,
    sysvar::instructions,
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::Token,
    token_interface::{
        burn_checked, mint_to, transfer_checked, BurnChecked, Mint, MintTo, TokenAccount,
        TokenInterface, TransferChecked,
    },
};
use solana_keccak_hasher::hash;
use solana_sdk_ids::secp256k1_program;

// Import necessary dependencies from Pyth
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

use crate::{
    constants::*,
    errors::OndoError,
    state::{
        Attestation, GMTokenManagerState, OndoUser, OracleSanityCheck, TokenLimit,
        USDonManagerState, Whitelist,
    },
    utils::{calculate_capacity_used, mul_div, normalize_decimals},
};
use anchor_lang::Discriminator;

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
    pub system_program: &'a Program<'info, System>,
    pub associated_token_program: &'a Program<'info, AssociatedToken>,
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

    /// Initializes a new attestation account with the provided attestation ID, timestamp, and bump.
    /// Marks the attestation ID as used to prevent replay attacks.
    /// # Arguments
    /// * `attestation_id` - A unique 16-byte identifier for the attestation.
    /// * `timestamp` - The timestamp when the attestation was created.
    /// * `bump` - The bump seed used for PDA derivation.
    #[inline(always)]
    pub fn initialize_attestation_account(
        &mut self,
        attestation_id: [u8; 16],
        timestamp: i64,
        bump: u8,
    ) -> Result<()> {
        // Check if the attestation account is uninitialized (lamports == 0)
        if self.attestation_id_account.data_is_empty() {
            // Calculate the required space for the attestation account
            let space = 8 + Attestation::INIT_SPACE;

            // Allocate space for the attestation account
            invoke_signed(
                &system_instruction::allocate(&self.attestation_id_account.key(), space as u64),
                &[self.attestation_id_account.to_account_info()],
                &[&[ATTESTATION_ID_SEED, attestation_id.as_ref(), &[bump]]],
            )?;

            // Fund the attestation account to be rent-exempt
            invoke(
                &system_instruction::transfer(
                    &self.user.key(),
                    &self.attestation_id_account.key(),
                    Rent::get()?
                        .minimum_balance(space)
                        .saturating_sub(self.attestation_id_account.lamports()),
                ),
                &[
                    self.user.to_account_info(),
                    self.attestation_id_account.to_account_info(),
                ],
            )?;

            // Assign the attestation account to the program
            invoke_signed(
                &system_instruction::assign(&self.attestation_id_account.key(), &crate::ID),
                &[self.attestation_id_account.to_account_info()],
                &[&[ATTESTATION_ID_SEED, attestation_id.as_ref(), &[bump]]],
            )?;

            // Borrow the attestation account data for writing
            let mut data = self.attestation_id_account.try_borrow_mut_data()?;

            // Write the discriminator
            data[0..8].copy_from_slice(Attestation::DISCRIMINATOR);

            // Create the attestation data
            let attestation = Attestation {
                attestation_id,
                creator: self.user.key(),
                created_at: timestamp,
                bump,
            };

            // Serialize the attestation data into the account
            attestation.serialize(&mut &mut data[8..])?;

            Ok(())
        } else {
            Err(OndoError::AttestationAlreadyUsed.into())
        }
    }

    /// Verifies the attestation signature using secp256k1.
    /// # Arguments
    /// * `chain_id` - A 32-byte identifier for the blockchain.
    /// * `attestation_id` - A unique 16-byte identifier for the attestation.
    /// * `side` - A byte indicating the side of the trade (e.g., buy/sell).
    /// * `price` - The price associated with the attestation.
    /// * `amount` - The amount associated with the attestation.
    /// * `expiration` - The expiration timestamp of the attestation.
    /// # Returns
    /// * `Result<()>` - Ok if the signature is valid, Err otherwise.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_attestation(
        &self,
        chain_id: [u8; 32],
        attestation_id: [u8; 16],
        side: u8,
        price: u64,
        amount: u64,
        expiration: i64,
    ) -> Result<()> {
        // Get the expected Ethereum address from the gmtoken manager state
        let eth_address = self.gmtoken_manager_state.attestation_signer_secp;
        // Check that the Ethereum address is initialized (not all zeros)
        require!(
            eth_address != [0u8; 20],
            OndoError::AttestationSignerEthAddressNotSet
        );

        // Calculate keccak256 hash of the quote
        let quote_hash = self.calculate_quote_hash(
            chain_id,
            attestation_id,
            side,
            self.user.key(),
            self.mint.key(),
            price,
            amount,
            expiration,
        );

        // Verify the secp256k1 signature using the instructions sysvar
        self.verify_secp256k1_ix(
            self.instructions.to_account_info().as_ref(),
            &quote_hash,
            eth_address,
        )?;

        msg!("✓ Attestation signature verified");

        Ok(())
    }

    /// Calculates the keccak256 hash of the quote parameters.
    /// # Arguments
    /// * `chain_id` - A 32-byte identifier for the blockchain.
    /// * `attestation_id` - A unique 16-byte identifier for the attestation.
    /// * `side` - A byte indicating the side of the trade (e.g., buy/sell).
    /// * `user` - The public key of the user.
    /// * `asset` - The public key of the asset (token mint).
    /// * `price` - The price associated with the attestation.
    /// * `amount` - The amount associated with the attestation.
    /// * `expiration` - The expiration timestamp of the attestation.
    /// # Returns
    /// * `[u8; 32]` - The keccak256 hash of the quote.
    #[allow(clippy::too_many_arguments)]
    fn calculate_quote_hash(
        &self,
        chain_id: [u8; 32],
        attestation_id: [u8; 16],
        side: u8,
        user: Pubkey,
        asset: Pubkey,
        price: u64,
        amount: u64,
        expiration: i64,
    ) -> [u8; 32] {
        // Concatenate:
        //   chain_id (32)
        // + attestation_id (16)
        // + side (1)
        // + user (32)
        // + asset (32)
        // + price (8)
        // + amount (8)
        // + expiration (8) = 137 bytes
        let mut quote = [0u8; 137];
        quote[0..32].copy_from_slice(&chain_id);
        quote[32..48].copy_from_slice(&attestation_id);
        quote[48] = side;
        quote[49..81].copy_from_slice(&user.to_bytes());
        quote[81..113].copy_from_slice(&asset.to_bytes());
        quote[113..121].copy_from_slice(&price.to_be_bytes());
        quote[121..129].copy_from_slice(&amount.to_be_bytes());
        quote[129..137].copy_from_slice(&expiration.to_be_bytes());

        // Calculate keccak256 hash of the quote
        hash(&quote).to_bytes()
    }

    /// Verifies the secp256k1 instruction in the transaction.
    /// # Arguments
    /// * `ix_sysvar` - The instructions sysvar account info.
    /// * `expected_digest32` - The expected 32-byte digest.
    /// * `expected_eth_address20` - The expected 20-byte Ethereum address.
    /// # Returns
    /// * `Result<()>` - Ok if the instruction is found and matches, Err otherwise.
    fn verify_secp256k1_ix(
        &self,
        ix_sysvar: &AccountInfo,
        expected_digest32: &[u8; 32],
        expected_eth_address20: [u8; 20],
    ) -> Result<()> {
        let current_ix_idx = instructions::load_current_index_checked(ix_sysvar)?;

        require_gt!(current_ix_idx, 0, SecpError::MissingOrMismatchedSecpIx);

        let ix_idx = current_ix_idx - 1;

        let secp_ix = instructions::load_instruction_at_checked(ix_idx as usize, ix_sysvar)?;

        require_keys_eq!(
            secp_ix.program_id,
            secp256k1_program::id(),
            SecpError::MissingOrMismatchedSecpIx
        );

        self.secp_matches(
            ix_idx as u8,
            &secp_ix,
            expected_digest32,
            expected_eth_address20,
        )?;

        Ok(())
    }

    /// secp_matches checks if the given secp256k1 instruction matches the expected digest and Ethereum address.
    /// The offsets struct points to signature(64+1 v), pubkey(64), and message digest(32).
    /// # Arguments
    /// * `ix` - The instruction to parse.
    /// * `digest` - The expected 32-byte digest.
    /// * `eth_addr` - The expected 20-byte Ethereum address.
    /// # Returns
    /// * `Result<bool>` - Ok(true) if the instruction matches, Err otherwise.
    fn secp_matches(
        &self,
        ix_idx: u8,
        ix: &Instruction,
        digest: &[u8; 32],
        eth_addr: [u8; 20],
    ) -> Result<bool> {
        let data = &ix.data;

        // First byte is number of signatures; require 1 for this simple flow.
        require!(!data.is_empty(), SecpError::MalformedSecpIx);
        require!(data[0] == 1, SecpError::WrongSigCount);

        // Skip the header and parse the first signature offsets (see program docs for exact layout).
        // We specifically extract the recovered 64-byte pubkey (uncompressed x||y) the 20-byte address.
        // Offset structure is 11 bytes starting at byte 1:
        // [sig_off(2), sig_ix(1), eth_off(2), eth_ix(1), msg_off(2), msg_len(2), msg_ix(1)]
        let rd = 1;
        require!(data.len() >= rd + 11, SecpError::MalformedSecpIx);

        // parse instruction data
        let sig_ix = data[rd + 2];
        let eth_off = u16::from_le_bytes([data[rd + 3], data[rd + 4]]) as usize;
        let eth_ix = data[rd + 5];
        let msg_off = u16::from_le_bytes([data[rd + 6], data[rd + 7]]) as usize;
        let msg_len = u16::from_le_bytes([data[rd + 8], data[rd + 9]]) as usize;
        let msg_ix = data[rd + 10];

        require!(msg_len == 32, SecpError::WrongDigestLen);
        require!(msg_off + msg_len <= data.len(), SecpError::MalformedSecpIx);
        require!(eth_off + 20 <= data.len(), SecpError::MalformedSecpIx);
        // only support "inline" mode, the instruction must refer to itself for the calldata.
        // that is, the KeccakSecp256k11111111111111111111111111111 instruction must contain the signature, eth_address, and msg
        require!(sig_ix == ix_idx, SecpError::MissingOrMismatchedSecpIx);
        require!(eth_ix == ix_idx, SecpError::MissingOrMismatchedSecpIx);
        require!(msg_ix == ix_idx, SecpError::MissingOrMismatchedSecpIx);

        let msg = &data[msg_off..msg_off + 32];
        let eth_addr_in_ix = &data[eth_off..eth_off + 20];

        // The secp256k1 precompile has already verified:
        // 1. signature is valid for keccak256(msg) where msg is the 32-byte digest
        // 2. The signature recovers to the expected ETH address
        // We just need to verify:
        // - msg (the digest in the secp instruction) matches our calculated digest
        // - ETH address in the instruction matches our expected ETH address
        require!(msg == digest, SecpError::DigestMismatch);
        require!(eth_addr_in_ix == eth_addr, SecpError::AddressMismatch);

        Ok(true)
    }

    /// Performs sanity checks on the token price and update time.
    /// # Arguments
    /// * `price` - The current price to check.
    /// * `current_timestamp` - The current timestamp.
    /// # Returns
    /// * `Result<()>` - Ok if all checks pass, Err otherwise.
    pub fn sanity_check(&mut self, price: u64, current_timestamp: i64) -> Result<()> {
        // Perform sanity checks on the token
        // Ensure the price is within a reasonable range of the last price
        let deviation = self
            .sanity_check_account
            .last_price
            .checked_mul(self.sanity_check_account.allowed_deviation_bps)
            .ok_or(OndoError::MathOverflow)?
            .checked_div(BASIS_POINTS_DIVISOR)
            .ok_or(OndoError::MathOverflow)?;

        // Calculate maximum acceptable price
        let max_price = self
            .sanity_check_account
            .last_price
            .checked_add(deviation)
            .ok_or(OndoError::MathOverflow)?;

        // Calculate minimum acceptable price
        let min_price = self
            .sanity_check_account
            .last_price
            .checked_sub(deviation)
            .ok_or(OndoError::MathOverflow)?;

        // Check if the price is within the allowed deviation range
        if price > max_price {
            msg!(
                "Price sanity check failed: price {} exceeds max_price {}. last_price={}, percentage_bp={}, deviation={}",
                price, max_price, self.sanity_check_account.last_price, self.sanity_check_account.allowed_deviation_bps, deviation
            );
            return Err(OndoError::PriceExceedsMaxDeviation.into());
        } else if price < min_price {
            msg!(
                "Price sanity check failed: price {} below min_price {}. last_price={}, percentage_bp={}, deviation={}",
                price, min_price, self.sanity_check_account.last_price, self.sanity_check_account.allowed_deviation_bps, deviation
            );
            return Err(OndoError::PriceBelowMinDeviation.into());
        }

        // Check time since last price update
        let elapsed_time = current_timestamp
            .checked_sub(self.sanity_check_account.price_last_updated)
            .ok_or(OndoError::MathOverflow)?;

        // Ensure the price data is recent enough
        if elapsed_time > self.sanity_check_account.max_time_delay {
            return Err(OndoError::MaxTimeDelayExceeded.into());
        }

        Ok(())
    }

    /// Performs rate limit checks at both token and user levels.
    /// # Arguments
    /// * `price` - The current price of the token.
    /// * `token_amount` - The amount of tokens involved in the transaction.
    /// * `current_timestamp` - The current timestamp.
    /// * `is_buy` - A boolean indicating if the transaction is a buy (true) or sell (false).
    /// # Returns
    /// * `Result<()>` - Ok if all checks pass, Err otherwise.
    fn rate_limit_check(
        &mut self,
        price: u64,
        token_amount: u64,
        current_timestamp: i64,
        is_buy: bool,
    ) -> Result<()> {
        // Round up: Conservative - counts more toward the rate limit
        let amount = mul_div(price, token_amount, PRICE_SCALING_FACTOR as u64, true)?;

        // Check token-level rate limit with linear decay
        self.check_token_rate_limit(amount, current_timestamp, is_buy)?;

        // Check user-level rate limit with linear decay
        self.check_user_rate_limit(amount, current_timestamp, is_buy)?;

        Ok(())
    }

    /// Checks and updates the token-level rate limit state.
    /// # Arguments
    /// * `amount` - The amount of tokens involved in the transaction.
    /// * `current_timestamp` - The current timestamp.
    /// * `is_buy` - A boolean indicating if the transaction is a buy (true) or sell (false).
    /// # Returns
    /// * `Result<()>` - Ok if the check passes, Err otherwise.
    #[inline(always)]
    fn check_token_rate_limit(
        &mut self,
        amount: u64,
        current_timestamp: i64,
        is_buy: bool,
    ) -> Result<()> {
        // Check if token-level rate limits are configured
        if let (Some(token_rate_limit), Some(token_limit_window)) = (
            self.token_limit_account.rate_limit,
            self.token_limit_account.limit_window,
        ) {
            let (token_capacity_used, token_last_updated) = if is_buy {
                (
                    self.token_limit_account
                        .mint_capacity_used
                        .ok_or(OndoError::DataMismatch)?,
                    self.token_limit_account
                        .mint_last_updated
                        .unwrap_or(current_timestamp),
                )
            } else {
                (
                    self.token_limit_account
                        .redeem_capacity_used
                        .ok_or(OndoError::DataMismatch)?,
                    self.token_limit_account
                        .redeem_last_updated
                        .unwrap_or(current_timestamp),
                )
            };

            // Calculate time since last update
            let time_since_last_update = current_timestamp
                .checked_sub(token_last_updated)
                .ok_or(OndoError::MathOverflow)?;

            // Calculate current capacity used with linear decay
            let current_token_capacity_used = calculate_capacity_used(
                time_since_last_update,
                token_limit_window,
                token_capacity_used,
                token_rate_limit,
            )?;

            // Calculate available capacity
            let available_token_capacity = if token_rate_limit > current_token_capacity_used {
                token_rate_limit
                    .checked_sub(current_token_capacity_used)
                    .ok_or(OndoError::MathOverflow)?
            } else {
                0
            };

            // Check if the requested amount exceeds available capacity
            if amount > available_token_capacity {
                msg!(
                    "Token rate limit exceeded: requested {} > available {}. rate_limit={}, capacity_used={}, window={}, time_since_update={}",
                    amount, available_token_capacity, token_rate_limit, current_token_capacity_used,
                    token_limit_window, time_since_last_update
                );
                return Err(OndoError::InvalidRateLimit.into());
            }

            // Update token rate limit state
            if is_buy {
                self.token_limit_account.mint_capacity_used = Some(
                    current_token_capacity_used
                        .checked_add(amount)
                        .ok_or(OndoError::MathOverflow)?,
                );
                self.token_limit_account.mint_last_updated = Some(current_timestamp);
            } else {
                self.token_limit_account.redeem_capacity_used = Some(
                    current_token_capacity_used
                        .checked_add(amount)
                        .ok_or(OndoError::MathOverflow)?,
                );
                self.token_limit_account.redeem_last_updated = Some(current_timestamp);
            }
        } else {
            // Token limits are not properly configured - fail the transaction
            return Err(OndoError::InvalidRateLimit.into());
        }

        Ok(())
    }

    /// Checks and updates the user-level rate limit state.
    /// # Arguments
    /// * `amount` - The amount of tokens involved in the transaction.
    /// * `current_timestamp` - The current timestamp.
    /// * `is_buy` - A boolean indicating if the transaction is a buy (true) or sell (false).
    /// # Returns
    /// * `Result<()>` - Ok if the check passes, Err otherwise.
    #[inline(always)]
    fn check_user_rate_limit(
        &mut self,
        amount: u64,
        current_timestamp: i64,
        is_buy: bool,
    ) -> Result<()> {
        if let (Some(user_rate_limit), Some(user_limit_window)) =
            (self.ondo_user.rate_limit, self.ondo_user.limit_window)
        {
            let (user_capacity_used, user_last_updated) = if is_buy {
                (
                    self.ondo_user
                        .mint_capacity_used
                        .ok_or(OndoError::DataMismatch)?,
                    self.ondo_user
                        .mint_last_updated
                        .unwrap_or(current_timestamp),
                )
            } else {
                (
                    self.ondo_user
                        .redeem_capacity_used
                        .ok_or(OndoError::DataMismatch)?,
                    self.ondo_user
                        .redeem_last_updated
                        .unwrap_or(current_timestamp),
                )
            };

            // Calculate time since last update
            let time_since_last_update = current_timestamp
                .checked_sub(user_last_updated)
                .ok_or(OndoError::MathOverflow)?;

            // Calculate current capacity used with linear decay
            let current_user_capacity_used = calculate_capacity_used(
                time_since_last_update,
                user_limit_window,
                user_capacity_used,
                user_rate_limit,
            )?;

            // Calculate available capacity
            let available_user_capacity = if user_rate_limit > current_user_capacity_used {
                user_rate_limit
                    .checked_sub(current_user_capacity_used)
                    .ok_or(OndoError::MathOverflow)?
            } else {
                0
            };

            // Check if the requested amount exceeds available capacity
            if amount > available_user_capacity {
                msg!(
                    "User rate limit exceeded: requested {} > available {}. rate_limit={}, capacity_used={}, window={}, time_since_update={}",
                    amount, available_user_capacity, user_rate_limit, current_user_capacity_used,
                    user_limit_window, time_since_last_update
                );
                return Err(OndoError::InvalidRateLimit.into());
            }

            // Update user rate limit state
            if is_buy {
                self.ondo_user.mint_capacity_used = Some(
                    current_user_capacity_used
                        .checked_add(amount)
                        .ok_or(OndoError::MathOverflow)?,
                );
                self.ondo_user.mint_last_updated = Some(current_timestamp);
            } else {
                self.ondo_user.redeem_capacity_used = Some(
                    current_user_capacity_used
                        .checked_add(amount)
                        .ok_or(OndoError::MathOverflow)?,
                );
                self.ondo_user.redeem_last_updated = Some(current_timestamp);
            }
        } else {
            // User limits are not properly configured - fail the transaction
            return Err(OndoError::InvalidRateLimit.into());
        }

        Ok(())
    }

    /// Swaps USDC tokens for USDon tokens (1:1 exchange).
    ///
    /// This method handles the conversion of USDC to USDon tokens with the following steps:
    /// 1. Validates input amount and retrieves current USDC price from the USDC price oracle
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
            self.usdc_oracle_sanity_check()?;
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

        // Return the calculated USDon amount for minting to user
        // Note: Actual USDon burn happens in the calling instruction
        Ok(normalized_amount_out)
    }

    /// Swaps USDon tokens for USDC tokens (1:1 exchange).
    ///
    /// This method handles the conversion of USDon to USDC tokens with the following steps:
    /// 1. Validates input amount and retrieves current USDC price from a USDC price oracle
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
            self.usdc_oracle_sanity_check()?;
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

    #[inline(always)]
    fn usdc_oracle_sanity_check(&self) -> Result<()> {
        // Retrieve the USDC price update account info
        let usdc_price_update_info = self
            .usdc_price_update
            .as_ref()
            .ok_or(OndoError::USDCOracleNotProvided)?
            .to_account_info();

        let usdc_price = match usdc_price_update_info.key() {
            USDC_PYTH_ORACLE_ADDRESS => {
                // Fetch the feed ID for the USDC token price from its hex representation.
                let usdc_feed_id: [u8; 32] = get_feed_id_from_hex(USDC_PYTH_ID)?;

                // Deserialize `usdc_price_update_info` account data into PriceUpdateV2 struct
                let data = usdc_price_update_info.try_borrow_data()?;
                let usdc_price_update_data = PriceUpdateV2::try_deserialize(&mut &data[..])?;

                // Retrieve current USDC/USD price from Pyth oracle with freshness validation
                // This ensures we're using recent price data to prevent stale price attacks
                let price_update_data = usdc_price_update_data.get_price_no_older_than(
                    &Clock::get()?,
                    self.usdon_manager_state.oracle_price_max_age,
                    &usdc_feed_id,
                )?;

                // Validate confidence interval is within threshold
                // Reject prices with high uncertainty to prevent using unreliable oracle data
                require!(price_update_data.price > 0, OndoError::InvalidPrice);

                // Check exponent is negative (Pyth convention)
                require!(
                    price_update_data.exponent < 0,
                    OndoError::InvalidPriceExponent
                );

                let conf = price_update_data.conf as u128;
                let price = price_update_data.price as u128;

                // Check: conf * 100 <= price * CONFIDENCE_THRESHOLD (equivalent to conf/price <= CONFIDENCE_THRESHOLD %)
                let conf_times_100 = conf.checked_mul(100).ok_or(OndoError::MathOverflow)?;
                let price_times_threshold = price
                    .checked_mul(CONFIDENCE_THRESHOLD)
                    .ok_or(OndoError::MathOverflow)?;

                require!(
                    conf_times_100 <= price_times_threshold,
                    OndoError::ConfidenceThresholdExceeded
                );

                let from_decimals = u8::try_from(-price_update_data.exponent)
                    .map_err(|_| OndoError::InvalidPriceExponent)?;

                normalize_decimals(
                    price_update_data.price as u64, // Safe to cast as we required price > 0 above
                    from_decimals,
                    USDC_PRICE_DECIMALS,
                    false,
                )?
            }
            _ => return err!(OndoError::USDCOracleNotImplemented),
        };

        // Validate that USDC price is above minimum threshold
        require_gte!(usdc_price, MIN_PRICE, OndoError::USDCBelowMinimumPrice);

        Ok(())
    }

    /// Verifies that the user is whitelisted by checking the whitelist account.
    /// # Returns
    /// * `Result<()>` - Ok if the user is whitelisted, Err(UserNotWhitelisted) otherwise.
    #[inline(always)]
    pub fn verify_whitelist(&self) -> Result<()> {
        let whitelist_data = self.whitelist.try_borrow_data()?;
        if whitelist_data.len() < 8 || whitelist_data[..8] != *Whitelist::DISCRIMINATOR {
            return Err(OndoError::UserNotWhitelisted.into());
        }
        Ok(())
    }

    /// Initializes the Ondo user account if it is not already initialized.
    /// Sets the owner, mint, rate limit, limit window, and bump values.
    /// # Arguments
    /// * `bump` - The bump seed used for PDA derivation.
    /// # Returns
    /// * `Result<()>` - Ok if initialization is successful or already initialized, Err otherwise
    #[inline(always)]
    pub fn initialize_ondo_user(&mut self, bump: u8) -> Result<()> {
        if self.ondo_user.owner != self.user.key() {
            self.ondo_user.owner = self.user.key();
            self.ondo_user.mint = self.mint.key();
            self.ondo_user.rate_limit = self.token_limit_account.default_user_rate_limit;
            self.ondo_user.limit_window = self.token_limit_account.default_user_limit_window;
            self.ondo_user.mint_capacity_used = Some(0);
            self.ondo_user.mint_last_updated = None;
            self.ondo_user.redeem_capacity_used = Some(0);
            self.ondo_user.redeem_last_updated = None;
            self.ondo_user.bump = bump;

            msg!("User initialized");
        }

        Ok(())
    }
}

/// Mints GM Tokens to the user's token account after verifying the attestation.
/// Transfers USDon or burns USDon based on the user's payment choice.
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
        OndoError::GMTokenMintingPaused
    );

    // Check if token-level minting is paused
    require!(
        !ctx.token_limit_account.minting_paused,
        OndoError::GMTokenMintingPaused
    );

    // Verify user is whitelisted
    ctx.verify_whitelist()?;

    // Validate input parameters
    require_gt!(amount, 0);
    require_gt!(price, 0);

    let current_timestamp = Clock::get()?.unix_timestamp;

    ctx.gmtoken_manager_state
        .check_is_valid_hours(current_timestamp)?;

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
    ctx.initialize_ondo_user(ondo_user_bump)?;

    // Create attestation account if it doesn't exist marking the attestation as consumed
    ctx.initialize_attestation_account(
        attestation_id,
        current_timestamp,
        attestation_id_account_bump,
    )?;

    // Verify the attestation signature
    ctx.verify_attestation(
        CHAIN_ID.to_bytes(),
        attestation_id,
        BUY,
        price,
        amount,
        expiration,
    )?;

    // Perform sanity check
    ctx.sanity_check(price, current_timestamp)?;

    // Check rate limit of the GM Token and user
    ctx.rate_limit_check(price, amount, current_timestamp, true)?;

    // Handle payment based on user's choice of USDon or USDC
    match is_usdon {
        true => {
            // Round up: Favours the protocol
            let amount_sent = mul_div(price, amount, PRICE_SCALING_FACTOR as u64, true)?;

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

            // Calculate the amount of USDC to be sent based on the price
            let amount_sent = mul_div(price, amount, PRICE_SCALING_FACTOR as u64, true)?;

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
        OndoError::GMTokenRedemptionPaused
    );

    // Check if token-level redemptions are paused
    require!(
        !ctx.token_limit_account.redemption_paused,
        OndoError::GMTokenRedemptionPaused
    );

    // Verify user is whitelisted
    ctx.verify_whitelist()?;

    // Validate input parameters
    require_gt!(amount, 0);
    require_gt!(price, 0);

    let current_timestamp = Clock::get()?.unix_timestamp;

    ctx.gmtoken_manager_state
        .check_is_valid_hours(current_timestamp)?;

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
    ctx.initialize_ondo_user(ondo_user_bump)?;

    // Create attestation account if it doesn't exist marking the attestation as consumed
    ctx.initialize_attestation_account(
        attestation_id,
        current_timestamp,
        attestation_id_account_bump,
    )?;

    // Verify the attestation signature
    ctx.verify_attestation(
        CHAIN_ID.to_bytes(),
        attestation_id,
        SELL,
        price,
        amount,
        expiration,
    )?;

    // Perform sanity check
    ctx.sanity_check(price, current_timestamp)?;

    // Check rate limit of the GM Token and user
    ctx.rate_limit_check(price, amount, current_timestamp, false)?;

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

/// Errors related to secp256k1 signature verification.
#[error_code]
pub enum SecpError {
    #[msg("Missing or mismatched secp256k1 verification instruction")]
    MissingOrMismatchedSecpIx,
    #[msg("Malformed secp256k1 instruction")]
    MalformedSecpIx,
    #[msg("Wrong signature count")]
    WrongSigCount,
    #[msg("Expected 32-byte hash")]
    WrongDigestLen,
    #[msg("Digest mismatch")]
    DigestMismatch,
    #[msg("Recovered address mismatch")]
    AddressMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Helper functions for sanity check testing
    fn validate_price_deviation(
        sanity_check: &OracleSanityCheck,
        price: u64,
    ) -> std::result::Result<(), OndoError> {
        let deviation = sanity_check
            .last_price
            .checked_mul(sanity_check.allowed_deviation_bps)
            .ok_or(OndoError::MathOverflow)?
            .checked_div(BASIS_POINTS_DIVISOR)
            .ok_or(OndoError::MathOverflow)?;

        let max_price = sanity_check
            .last_price
            .checked_add(deviation)
            .ok_or(OndoError::MathOverflow)?;

        let min_price = sanity_check
            .last_price
            .checked_sub(deviation)
            .ok_or(OndoError::MathOverflow)?;

        if price > max_price {
            return Err(OndoError::PriceExceedsMaxDeviation);
        } else if price < min_price {
            return Err(OndoError::PriceBelowMinDeviation);
        }

        Ok(())
    }

    fn validate_time_delay(
        sanity_check: &OracleSanityCheck,
        current_timestamp: i64,
    ) -> std::result::Result<(), OndoError> {
        let elapsed_time = current_timestamp
            .checked_sub(sanity_check.price_last_updated)
            .ok_or(OndoError::MathOverflow)?;

        if elapsed_time > sanity_check.max_time_delay {
            return Err(OndoError::MaxTimeDelayExceeded);
        }

        Ok(())
    }

    fn validate_sanity_check(
        sanity_check: &OracleSanityCheck,
        price: u64,
        current_timestamp: i64,
    ) -> std::result::Result<(), OndoError> {
        validate_price_deviation(sanity_check, price)?;
        validate_time_delay(sanity_check, current_timestamp)?;
        Ok(())
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
        assert!(matches!(result, Err(OndoError::PriceExceedsMaxDeviation)));

        // Test price slightly below lower bound (should fail)
        let result = validate_price_deviation(&sanity_check, 949_999_999);
        assert!(result.is_err());
        assert!(matches!(result, Err(OndoError::PriceBelowMinDeviation)));
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
        let result = validate_time_delay(&sanity_check, current_timestamp);
        assert!(result.is_ok());

        // Test exactly at time limit (should pass)
        // 7 days = 7 * 86400 = 604800 seconds
        let current_timestamp = base_timestamp + (7 * SECONDS_PER_DAY);
        let result = validate_time_delay(&sanity_check, current_timestamp);
        assert!(result.is_ok());

        // Test beyond time limit (should fail)
        // 8 days = 8 * 86400 = 691200 seconds
        let current_timestamp = base_timestamp + (8 * SECONDS_PER_DAY);
        let result = validate_time_delay(&sanity_check, current_timestamp);
        assert!(result.is_err());
        assert!(matches!(result, Err(OndoError::MaxTimeDelayExceeded)));
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
        let result = validate_sanity_check(&sanity_check, 1_020_000_000, current_timestamp);
        assert!(result.is_ok());

        // Invalid price but valid time (should fail on price)
        let result = validate_sanity_check(&sanity_check, 1_100_000_000, current_timestamp);
        assert!(result.is_err());
        assert!(matches!(result, Err(OndoError::PriceExceedsMaxDeviation)));

        // Valid price but invalid time (should fail on time)
        let current_timestamp = base_timestamp + (10 * SECONDS_PER_DAY);
        let result = validate_sanity_check(&sanity_check, 1_020_000_000, current_timestamp);
        assert!(result.is_err());
        assert!(matches!(result, Err(OndoError::MaxTimeDelayExceeded)));
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

        let result = validate_time_delay(&sanity_check, base_timestamp);
        assert!(result.is_ok());

        // Even 1 second later should fail
        let result = validate_time_delay(&sanity_check, base_timestamp + 1);
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
        let result = validate_time_delay(&sanity_check, current_timestamp);
        assert!(result.is_ok());

        let current_timestamp = base_timestamp + (365 * 10 * SECONDS_PER_DAY) + 1;
        let result = validate_time_delay(&sanity_check, current_timestamp);
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
                prop_assert!(matches!(result, Err(OndoError::PriceExceedsMaxDeviation)));
            } else if new_price < min_price {
                prop_assert!(result.is_err());
                prop_assert!(matches!(result, Err(OndoError::PriceBelowMinDeviation)));
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
            let result = validate_time_delay(&sanity_check, current_timestamp);

            if seconds_elapsed > max_time_delay {
                prop_assert!(result.is_err());
                prop_assert!(matches!(result, Err(OndoError::MaxTimeDelayExceeded)));
            } else {
                prop_assert!(result.is_ok());
            }
        }
    }
}
