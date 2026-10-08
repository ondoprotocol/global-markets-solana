use anchor_lang::prelude::*;
use anchor_spl::token_interface::{mint_to, Mint, MintTo, TokenAccount, TokenInterface};

use crate::{
    constants::{MAX_MINT_AMOUNT, PRICE_SCALING_FACTOR},
    errors::OndoError,
    state::OracleSanityCheck,
    utils::mul_div,
};

pub struct MintParams<'a, 'info> {
    pub oracle_sanity_check: &'a OracleSanityCheck,
    pub mint: &'a InterfaceAccount<'info, Mint>,
    pub mint_authority: &'a AccountInfo<'info>,
    pub destination: &'a InterfaceAccount<'info, TokenAccount>,
    pub token_program: &'a Interface<'info, TokenInterface>,
    pub seed: &'static [u8],
    pub bump: u8,
}

/// Mint GM or portfolio tokens using a fresh reference price and a $10 million notional cap.
/// Callers enforce roles and pause flags; this helper does not consume trading rate limits.
/// # Arguments
/// * `params` - Accounts, reference price, and mint authority PDA seed and bump.
/// * `amount` - The amount of tokens to mint (must be greater than 0)
#[inline(always)]
pub fn mint(params: MintParams, amount: u64) -> Result<()> {
    // Validate amount is greater than 0
    require_gt!(amount, 0, OndoError::InvalidAmount);

    let MintParams {
        oracle_sanity_check,
        mint,
        mint_authority,
        destination,
        token_program,
        seed,
        bump,
    } = params;

    oracle_sanity_check.validate_freshness(Clock::get()?.unix_timestamp)?;

    // Calculate notional USD value: (amount × price) / PRICE_SCALING_FACTOR
    let notional_usd = mul_div(
        amount,
        oracle_sanity_check.last_price,
        PRICE_SCALING_FACTOR as u64,
        true,
    )?;

    // Validate notional USD value does not exceed $10 million
    require_gte!(
        MAX_MINT_AMOUNT,
        notional_usd,
        OndoError::AmountExceedsMaxMintAmount
    );

    // Mint GM/Portfolio Tokens to the destination account
    // using the mint authority PDA as signer
    mint_to(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            MintTo {
                mint: mint.to_account_info(),
                to: destination.to_account_info(),
                authority: mint_authority.to_account_info(),
            },
            &[&[seed, &[bump]]],
        ),
        amount,
    )
}
