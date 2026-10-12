use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use super::{mint, MintParams};
use crate::{
    constants::{
        GMTOKEN_MANAGER_STATE_SEED, MINT_AUTHORITY_SEED, ORACLE_SANITY_CHECK_SEED,
        TOKEN_LIMIT_ACCOUNT_SEED, USDON_MANAGER_STATE_SEED,
    },
    errors::OndoError,
    instructions::TokenKind,
    state::{
        GMTokenManagerState, OracleSanityCheck, RoleType, Roles, TokenLimit, USDonManagerState,
    },
};

/// Mint GM Tokens
/// Requires `MINTER_ROLE_GMTOKEN` role
#[derive(Accounts)]
pub struct GMTokenMinter<'info> {
    /// Pays for destination account if needed
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to mint GM Tokens,
    pub authority: Signer<'info>,

    /// The user receiving the minted tokens
    /// CHECK: The authority of the destination token account, enforced by `associated_token` constraint
    pub user: UncheckedAccount<'info>,

    /// The `Roles` account verifying the authority has the `MINTER_ROLE_GMTOKEN` role
    /// # PDA Seeds
    /// - `MINTER_ROLE_GMTOKEN`
    /// - The authority's address
    #[account(
        seeds = [RoleType::MINTER_ROLE_GMTOKEN, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `OracleSanityCheck` account supplying the price used for the notional cap
    /// # PDA Seeds
    /// - `ORACLE_SANITY_CHECK_SEED`
    /// - Mint address
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

    /// The mint authority PDA
    /// # PDA Seeds
    /// - `MINT_AUTHORITY_SEED`
    ///
    /// CHECK: The seeds constraint validates this authority PDA; no account data is read.
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The GM Token mint to mint from
    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program,
        constraint = mint.key() != usdon_manager_state.usdon_mint @ OndoError::InvalidInputMint
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// The destination token account to mint tokens to
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = user,
    )]
    pub destination: InterfaceAccount<'info, TokenAccount>,

    /// The `USDonManagerState` account for validation
    /// # PDA Seeds
    /// - `USDON_MANAGER_STATE_SEED`
    #[account(
        seeds = [USDON_MANAGER_STATE_SEED],
        bump = usdon_manager_state.bump,
    )]
    pub usdon_manager_state: Account<'info, USDonManagerState>,

    #[account(
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager.bump,
    )]
    pub gmtoken_manager: Account<'info, GMTokenManagerState>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,
    /// The associated token program
    pub associated_token_program: Program<'info, AssociatedToken>,
    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> GMTokenMinter<'info> {
    const SEED: &'static [u8] = TokenKind::GM.seed();

    /// Mint GM tokens to a user's account
    /// # Arguments
    /// * `amount` - The amount of tokens to mint (must be greater than 0)
    /// * `bump` - The PDA bump for the mint authority
    /// # Returns
    /// * `Result<()>` - Ok if tokens are successfully minted, Err otherwise
    pub fn mint_gm(&mut self, amount: u64, bump: u8) -> Result<()> {
        require!(
            !self.gmtoken_manager.minting_paused && !self.token_limit.minting_paused,
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
