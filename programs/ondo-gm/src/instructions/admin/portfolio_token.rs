use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use super::{mint, MintParams};
use crate::{
    constants::{
        ORACLE_SANITY_CHECK_SEED, PORTFOLIO_MINT_AUTHORITY_SEED,
        PORTFOLIO_TOKEN_MANAGER_STATE_SEED, TOKEN_LIMIT_ACCOUNT_SEED, USDON_MANAGER_STATE_SEED,
    },
    errors::OndoError,
    instructions::TokenKind,
    state::{
        OracleSanityCheck, PortfolioTokenManagerState, RoleType, Roles, TokenLimit,
        USDonManagerState,
    },
};

/// Mint Portfolio Tokens
/// Requires `MINTER_ROLE_PORTFOLIO_TOKEN`
#[derive(Accounts)]
pub struct PortfolioTokenMinter<'info> {
    /// Pays for destination account if needed
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to mint Portfolio Tokens,
    pub authority: Signer<'info>,

    /// CHECK: The authority of the destination token account, enforced by `associated_token` constraint
    pub user: UncheckedAccount<'info>,

    /// The `Roles` account verifying the authority has the `MINTER_ROLE_PORTFOLIO_TOKEN` role
    #[account(
        seeds = [RoleType::MINTER_ROLE_PORTFOLIO_TOKEN, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `OracleSanityCheck` account supplying the price used for the notional cap
    #[account(
        seeds = [ORACLE_SANITY_CHECK_SEED, mint.key().as_ref()],
        bump = oracle_sanity_check.bump,
        has_one = mint @ OndoError::InvalidInputMint
    )]
    pub oracle_sanity_check: Account<'info, OracleSanityCheck>,

    #[account(
        seeds = [TOKEN_LIMIT_ACCOUNT_SEED, mint.key().as_ref()],
        bump = token_limit.bump,
    )]
    pub token_limit: Account<'info, TokenLimit>,

    /// CHECK: The seeds constraint validates this authority PDA; no account data is read.
    #[account(
        seeds = [PORTFOLIO_MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = user,
    )]
    pub destination: InterfaceAccount<'info, TokenAccount>,

    /// The `USDonManagerState` account for validation
    #[account(
        seeds = [USDON_MANAGER_STATE_SEED],
        bump = usdon_manager_state.bump,
    )]
    pub usdon_manager_state: Account<'info, USDonManagerState>,

    #[account(
        seeds = [PORTFOLIO_TOKEN_MANAGER_STATE_SEED],
        bump = portfolio_token_manager.bump,
    )]
    pub portfolio_token_manager: Account<'info, PortfolioTokenManagerState>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> PortfolioTokenMinter<'info> {
    const SEED: &'static [u8] = TokenKind::Portfolio.seed();

    pub fn mint_portfolio(&mut self, amount: u64, bump: u8) -> Result<()> {
        require!(
            !self.portfolio_token_manager.minting_paused && !self.token_limit.minting_paused,
            OndoError::TokenMintingPaused
        );

        mint(
            MintParams {
                oracle_sanity_check: &self.oracle_sanity_check,
                mint: &self.mint,
                mint_authority: &self.mint_authority,
                destination: &self.destination,
                token_program: &self.token_program,
                seed: Self::SEED,
                bump,
            },
            amount,
        )
    }
}
