pub mod admin;
pub mod close_attestation_account;
pub mod portfolio_token_manager;
pub mod token_manager;
pub mod usdc_swap_context;
pub mod usdon_swap_context;

pub use admin::*;
pub use close_attestation_account::*;
pub use portfolio_token_manager::*;
pub use token_manager::*;
pub use usdc_swap_context::*;
pub use usdon_swap_context::*;

use anchor_spl::associated_token::spl_associated_token_account::solana_program::program::{
    invoke, invoke_signed,
};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

use crate::{
    constants::{
        ATTESTATION_ID_SEED, CONFIDENCE_THRESHOLD, MINT_AUTHORITY_SEED, MIN_PRICE,
        PORTFOLIO_MINT_AUTHORITY_SEED, USDC_PRICE_DECIMALS, USDC_PYTH_ID, USDC_PYTH_ORACLE_ADDRESS,
    },
    errors::OndoError,
    state::{Attestation, USDonManagerState, Whitelist},
    utils::{calculate_quote_hash, normalize_decimals, verify_secp256k1_ix},
};
use anchor_lang::prelude::*;

#[derive(AnchorDeserialize, AnchorSerialize)]
pub enum TokenKind {
    GM = 0,
    Portfolio = 1,
}

impl TokenKind {
    /// The PDA that holds this kind's mint authority, and signs its mint and
    /// pausable-extension CPIs.
    pub const fn seed(&self) -> &'static [u8] {
        match self {
            TokenKind::GM => MINT_AUTHORITY_SEED,
            TokenKind::Portfolio => PORTFOLIO_MINT_AUTHORITY_SEED,
        }
    }
}

/// Initializes a new attestation account with the provided attestation ID, timestamp, and bump.
/// Marks the attestation ID as used to prevent replay attacks.
/// # Arguments
/// * `attestation_id` - A unique 16-byte identifier for the attestation.
/// * `timestamp` - The timestamp when the attestation was created.
/// * `bump` - The bump seed used for PDA derivation.
#[inline(always)]
pub fn create_attestation_account<'a>(
    user: &AccountInfo<'a>,
    attestation_account: &AccountInfo<'a>,
    attestation_id: [u8; 16],
    timestamp: i64,
    bump: u8,
) -> Result<()> {
    // Empty data means uninitialized, including a PDA prefunded with lamports
    if attestation_account.data_is_empty() {
        // Calculate the required space for the attestation account
        let space = 8 + Attestation::INIT_SPACE;

        // Allocate space for the attestation account
        invoke_signed(
            &system_instruction::allocate(&attestation_account.key(), space as u64),
            &[attestation_account.to_account_info()],
            &[&[ATTESTATION_ID_SEED, attestation_id.as_ref(), &[bump]]],
        )?;

        // Fund the attestation account to be rent-exempt
        invoke(
            &system_instruction::transfer(
                &user.key(),
                &attestation_account.key(),
                Rent::get()?
                    .minimum_balance(space)
                    .saturating_sub(attestation_account.lamports()),
            ),
            &[
                user.to_account_info(),
                attestation_account.to_account_info(),
            ],
        )?;

        // Assign the attestation account to the program
        invoke_signed(
            &system_instruction::assign(&attestation_account.key(), &crate::ID),
            &[attestation_account.to_account_info()],
            &[&[ATTESTATION_ID_SEED, attestation_id.as_ref(), &[bump]]],
        )?;

        // Borrow the attestation account data for writing
        let mut data = attestation_account.try_borrow_mut_data()?;

        // Write the discriminator
        data[0..8].copy_from_slice(Attestation::DISCRIMINATOR);

        // Create the attestation data
        let attestation = Attestation {
            attestation_id,
            creator: user.key(),
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
    chain_id: [u8; 32],
    attestation_id: [u8; 16],
    side: u8,
    price: u64,
    amount: u64,
    expiration: i64,
    user: Pubkey,
    mint: Pubkey,
    instructions: &AccountInfo,
    eth_address: [u8; 20],
) -> Result<()> {
    // Check that the Ethereum address is initialized (not all zeros)
    require!(
        eth_address != [0u8; 20],
        OndoError::AttestationSignerEthAddressNotSet
    );

    // Calculate keccak256 hash of the quote
    let quote_hash = calculate_quote_hash(
        chain_id,
        attestation_id,
        side,
        user,
        mint,
        price,
        amount,
        expiration,
    );

    // Verify the secp256k1 signature using the instructions sysvar
    verify_secp256k1_ix(instructions, &quote_hash, eth_address)?;

    msg!("✓ Attestation signature verified");

    Ok(())
}

#[inline(always)]
fn verify_whitelist(data: &[u8]) -> Result<()> {
    if data.len() < 8 || data[..8] != *Whitelist::DISCRIMINATOR {
        return Err(OndoError::UserNotWhitelisted.into());
    }
    Ok(())
}

#[inline(always)]
fn usdc_oracle_sanity_check(
    usdc_price_update_info: &AccountInfo,
    usdon_manager: &USDonManagerState,
) -> Result<()> {
    let usdc_price = match usdc_price_update_info.key() {
        USDC_PYTH_ORACLE_ADDRESS => {
            let usdc_feed_id: [u8; 32] = get_feed_id_from_hex(USDC_PYTH_ID)?;
            let data = usdc_price_update_info.try_borrow_data()?;
            let usdc_price_update_data = PriceUpdateV2::try_deserialize(&mut &data[..])?;

            // Retrieve current USDC/USD price from Pyth oracle with freshness validation
            // This ensures we're using recent price data to prevent stale price attacks
            let price_update_data = usdc_price_update_data.get_price_no_older_than(
                &Clock::get()?,
                usdon_manager.oracle_price_max_age,
                &usdc_feed_id,
            )?;

            // Require a positive price before confidence and decimal calculations
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
