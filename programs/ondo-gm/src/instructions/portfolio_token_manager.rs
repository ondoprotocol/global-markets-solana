use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::ID as INSTRUCTIONS_ID;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::Token,
    token_2022::{burn_checked, mint_to, transfer_checked, BurnChecked, MintTo, TransferChecked},
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    constants::*,
    errors::OndoError,
    events::TradeExecuted,
    instructions::{
        create_attestation_account, usdc_oracle_sanity_check, verify_attestation, verify_whitelist,
    },
    state::{
        OndoUser, OracleSanityCheck, PortfolioTokenManagerState, TokenLimit, USDonManagerState,
    },
    utils::{mul_div, normalize_decimals},
};

#[event_cpi]
#[derive(Accounts)]
#[instruction(attestation_id: [u8; 16])]
pub struct PortfolioTokenManager<'info> {
    /// pays for account creation if needed
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: The seeds constraint validates this authority PDA; no account data is read.
    #[account(
        seeds = [PORTFOLIO_MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + OndoUser::INIT_SPACE,
        seeds = [ONDO_USER_SEED, user.key().as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub ondo_user: Box<Account<'info, OndoUser>>,

    #[account(
        mut,
        seeds = [TOKEN_LIMIT_ACCOUNT_SEED, mint.key().as_ref()],
        bump = token_limit.bump,
    )]
    pub token_limit: Box<Account<'info, TokenLimit>>,

    #[account(
        seeds = [ORACLE_SANITY_CHECK_SEED, mint.key().as_ref()],
        bump = oracle_sanity_check.bump,
    )]
    pub oracle_sanity_check: Box<Account<'info, OracleSanityCheck>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Seeds constraint validates PDA address.
    /// Existence means the attestation has been used.
    #[account(
        mut,
        seeds = [ATTESTATION_ID_SEED, attestation_id.as_ref()],
        bump,
    )]
    pub attestation: UncheckedAccount<'info>,

    /// CHECK: Seeds constraint validates PDA address.
    /// Validated in instruction handler - returns UserNotWhitelisted if not initialized.
    #[account(
        seeds = [WHITELIST_SEED, user.key().as_ref()],
        bump,
    )]
    pub whitelist: UncheckedAccount<'info>,

    /// The token program (Token2022)
    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The legacy SPL Token program
    pub spl_token_program: Program<'info, Token>,

    /// The oracle price update account for USDC/USD price feed
    /// When the guard is enabled, checks USDC price, confidence, and freshness.
    /// This guard does not change the attested portfolio settlement price.
    /// CHECK: Constraint using `has_one` in usdon_manager.
    /// `has_one` ensures that this account matches the expected oracle account stored in the `usdon_manager`.
    pub usdc_price_update: UncheckedAccount<'info>,

    /// The USDC vault storing USDC tokens received from users during swaps
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = usdon_manager,
        associated_token::token_program = spl_token_program,
        constraint = usdc_vault.key() == usdon_manager.usdc_vault
    )]
    pub usdc_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[cfg(not(any(feature = "mainnet", feature = "testnet")))]
    #[account(
        mint::token_program = spl_token_program,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[cfg(any(feature = "mainnet", feature = "testnet"))]
    #[account(
        mint::token_program = spl_token_program,
        constraint = usdc_mint.key() == USDC_MINT
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub user_usdc_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Used as authority for vault operations
    #[account(
        has_one = usdc_price_update,
        seeds = [USDON_MANAGER_STATE_SEED],
        bump = usdon_manager.bump,
    )]
    pub usdon_manager: Box<Account<'info, USDonManagerState>>,

    #[account(
        mut,
        seeds = [PORTFOLIO_TOKEN_MANAGER_STATE_SEED],
        bump = portfolio_token_manager.bump,
    )]
    pub portfolio_token_manager: Box<Account<'info, PortfolioTokenManagerState>>,

    /// CHECK: Sysvar account for instruction introspection
    #[account(address = INSTRUCTIONS_ID)]
    pub instructions: UncheckedAccount<'info>,
}

impl<'info> PortfolioTokenManager<'info> {
    pub fn validate(&self) -> Result<()> {
        // Validate the user's USDC token account
        require_keys_eq!(
            self.user_usdc_token_account.mint,
            self.usdc_mint.key(),
            OndoError::InvalidTokenAccount
        );
        require_keys_eq!(
            self.user_usdc_token_account.owner,
            self.user.key(),
            OndoError::InvalidTokenAccount
        );
        require_keys_eq!(
            *self.user_usdc_token_account.to_account_info().owner,
            self.spl_token_program.key(),
            OndoError::InvalidTokenAccount
        );

        Ok(())
    }
}

pub fn mint_with_attestation(
    ctx: Context<PortfolioTokenManager>,
    attestation_id: [u8; 16],
    price: u64,
    amount: u64,
    expiration: i64,
) -> Result<()> {
    ctx.accounts.validate()?;

    let PortfolioTokenManager {
        user,
        mint,
        mint_authority,
        ondo_user,
        token_limit,
        oracle_sanity_check,
        user_ata,
        attestation,
        whitelist,
        token_program,
        spl_token_program,
        usdc_price_update,
        usdc_vault,
        usdc_mint,
        user_usdc_token_account,
        usdon_manager,
        portfolio_token_manager,
        instructions,
        ..
    } = ctx.accounts;

    require!(
        !portfolio_token_manager.minting_paused && !token_limit.minting_paused,
        OndoError::TokenMintingPaused
    );

    require_gt!(amount, 0, OndoError::InvalidAmount);
    require_gt!(price, 0, OndoError::InvalidPrice);

    let current_timestamp = Clock::get()?.unix_timestamp;
    require!(
        current_timestamp < expiration,
        OndoError::AttestationExpired
    );
    require!(
        expiration - current_timestamp <= MAX_ATTESTATION_EXPIRATION,
        OndoError::AttestationExpirationTooLarge
    );

    verify_whitelist(&whitelist.try_borrow_data()?)?;

    ondo_user.initialize(
        &user.key(),
        &mint.key(),
        token_limit.default_user_rate_limit,
        token_limit.default_user_limit_window,
        ctx.bumps.ondo_user,
    );

    create_attestation_account(
        &user.to_account_info(),
        &attestation.to_account_info(),
        attestation_id,
        current_timestamp,
        ctx.bumps.attestation,
    )?;

    verify_attestation(
        CHAIN_ID.to_bytes(),
        attestation_id,
        BUY,
        price,
        amount,
        expiration,
        user.key(),
        mint.key(),
        &instructions.to_account_info(),
        portfolio_token_manager.attestation_signer_secp,
    )?;

    oracle_sanity_check.sanity_check(price, current_timestamp)?;

    let amount_sent = mul_div(price, amount, PRICE_SCALING_FACTOR as u64, true)?;
    require_gt!(amount_sent, 0);

    token_limit.check_and_update_limit(amount_sent, current_timestamp, true)?;
    ondo_user.check_and_update_limit(amount_sent, current_timestamp, true)?;

    // Perform sanity checks on the USDC token
    if usdon_manager.oracle_price_enabled {
        usdc_oracle_sanity_check(usdc_price_update, usdon_manager)?;
    }

    // transfer USDC
    transfer_checked(
        CpiContext::new(
            spl_token_program.to_account_info(),
            TransferChecked {
                from: user_usdc_token_account.to_account_info(),
                mint: usdc_mint.to_account_info(),
                to: usdc_vault.to_account_info(),
                authority: user.to_account_info(),
            },
        ),
        normalize_decimals(amount_sent, mint.decimals, usdc_mint.decimals, true)?,
        usdc_mint.decimals,
    )?;

    // mint portfolio token
    mint_to(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            MintTo {
                mint: mint.to_account_info(),
                to: user_ata.to_account_info(),
                authority: mint_authority.to_account_info(),
            },
            &[&[PORTFOLIO_MINT_AUTHORITY_SEED, &[ctx.bumps.mint_authority]]],
        ),
        amount,
    )?;

    emit_cpi!(TradeExecuted {
        execution_id: portfolio_token_manager.next_execution_id()?,
    });

    Ok(())
}

pub fn redeem_with_attestation(
    ctx: Context<PortfolioTokenManager>,
    attestation_id: [u8; 16],
    price: u64,
    amount: u64,
    expiration: i64,
) -> Result<()> {
    ctx.accounts.validate()?;

    let PortfolioTokenManager {
        user,
        mint,
        ondo_user,
        token_limit,
        oracle_sanity_check,
        user_ata,
        attestation,
        whitelist,
        token_program,
        spl_token_program,
        usdc_price_update,
        usdc_vault,
        usdc_mint,
        user_usdc_token_account,
        usdon_manager,
        portfolio_token_manager,
        instructions,
        ..
    } = ctx.accounts;

    require!(
        !portfolio_token_manager.redemption_paused && !token_limit.redemption_paused,
        OndoError::TokenRedemptionPaused
    );

    require_gt!(amount, 0, OndoError::InvalidAmount);
    require_gt!(price, 0, OndoError::InvalidPrice);

    let current_timestamp = Clock::get()?.unix_timestamp;
    require!(
        current_timestamp < expiration,
        OndoError::AttestationExpired
    );
    require!(
        expiration - current_timestamp <= MAX_ATTESTATION_EXPIRATION,
        OndoError::AttestationExpirationTooLarge
    );

    verify_whitelist(&whitelist.try_borrow_data()?)?;

    ondo_user.initialize(
        &user.key(),
        &mint.key(),
        token_limit.default_user_rate_limit,
        token_limit.default_user_limit_window,
        ctx.bumps.ondo_user,
    );

    create_attestation_account(
        &user.to_account_info(),
        &attestation.to_account_info(),
        attestation_id,
        current_timestamp,
        ctx.bumps.attestation,
    )?;

    verify_attestation(
        CHAIN_ID.to_bytes(),
        attestation_id,
        SELL,
        price,
        amount,
        expiration,
        user.key(),
        mint.key(),
        &instructions.to_account_info(),
        portfolio_token_manager.attestation_signer_secp,
    )?;

    oracle_sanity_check.sanity_check(price, current_timestamp)?;

    let rate_limited_amount = mul_div(price, amount, PRICE_SCALING_FACTOR as u64, true)?;
    token_limit.check_and_update_limit(rate_limited_amount, current_timestamp, false)?;
    ondo_user.check_and_update_limit(rate_limited_amount, current_timestamp, false)?;

    let usdc_amount = mul_div(price, amount, PRICE_SCALING_FACTOR as u64, false)?;
    let usdc_amount_normalized =
        normalize_decimals(usdc_amount, mint.decimals, usdc_mint.decimals, false)?;

    require_gt!(usdc_amount_normalized, 0, OndoError::InvalidAmount);

    // Perform sanity checks on the USDC token
    if usdon_manager.oracle_price_enabled {
        usdc_oracle_sanity_check(usdc_price_update, usdon_manager)?;
    }

    // transfer USDC
    transfer_checked(
        CpiContext::new_with_signer(
            spl_token_program.to_account_info(),
            TransferChecked {
                from: usdc_vault.to_account_info(),
                mint: usdc_mint.to_account_info(),
                to: user_usdc_token_account.to_account_info(),
                authority: usdon_manager.to_account_info(),
            },
            &[&[USDON_MANAGER_STATE_SEED, &[usdon_manager.bump]]],
        ),
        usdc_amount_normalized,
        usdc_mint.decimals,
    )?;

    // burn portfolio token
    burn_checked(
        CpiContext::new(
            token_program.to_account_info(),
            BurnChecked {
                mint: mint.to_account_info(),
                from: user_ata.to_account_info(),
                authority: user.to_account_info(),
            },
        ),
        amount,
        mint.decimals,
    )?;

    emit_cpi!(TradeExecuted {
        execution_id: portfolio_token_manager.next_execution_id()?,
    });

    Ok(())
}
