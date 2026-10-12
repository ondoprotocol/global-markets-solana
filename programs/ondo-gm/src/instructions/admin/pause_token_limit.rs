//! Pause or resume protocol minting/redemption for a single mint by updating its `TokenLimit` flags.
//!
//! `PauserRoleTokenLimit` can pause; `AdminRoleTokenLimit` can pause or resume.
//! The target PDA is keyed by mint, while the authority's role PDA is program-wide.
//! These flags are independent of global manager pauses and Token-2022's Pausable extension.
//! Clearing one layer does not clear the others.

use anchor_lang::prelude::*;

use crate::{
    constants::TOKEN_LIMIT_ACCOUNT_SEED,
    events::{TokenMintingPaused, TokenRedemptionPaused},
    state::{RoleType, Roles, TokenLimit},
};

/// Set or clear per-mint pause flags using `AdminRoleTokenLimit`.
#[derive(Accounts)]
pub struct TokenLimitAdminPauser<'info> {
    /// The account with the authority to execute the pause/unpause operation
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_TOKEN_LIMIT` role
    #[account(
        seeds = [RoleType::ADMIN_ROLE_TOKEN_LIMIT, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `TokenLimit` account of the token being paused/resumed
    #[account(
        mut,
        seeds = [TOKEN_LIMIT_ACCOUNT_SEED, token_limit_account.mint.as_ref()],
        bump = token_limit_account.bump,
    )]
    pub token_limit_account: Account<'info, TokenLimit>,
}

impl<'info> TokenLimitAdminPauser<'info> {
    /// Set `redemption_paused` to true and emit `TokenRedemptionPaused` for this mint.
    pub fn pause_token_redemption(&mut self) -> Result<()> {
        self.token_limit_account.redemption_paused = true;

        emit!(TokenRedemptionPaused {
            is_paused: true,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    /// Set `minting_paused` to true and emit `TokenMintingPaused` for this mint.
    pub fn pause_token_minting(&mut self) -> Result<()> {
        self.token_limit_account.minting_paused = true;

        emit!(TokenMintingPaused {
            is_paused: true,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    /// Clear `redemption_paused` and emit `TokenRedemptionPaused` with `is_paused = false`.
    pub fn resume_token_redemption(&mut self) -> Result<()> {
        self.token_limit_account.redemption_paused = false;

        emit!(TokenRedemptionPaused {
            is_paused: false,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    /// Clear `minting_paused` and emit `TokenMintingPaused` with `is_paused = false`.
    pub fn resume_token_minting(&mut self) -> Result<()> {
        self.token_limit_account.minting_paused = false;

        emit!(TokenMintingPaused {
            is_paused: false,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }
}

/// Pause minting/redemption for a token via its TokenLimit account
/// Requires `PAUSER_ROLE_TOKEN_LIMIT` role
#[derive(Accounts)]
pub struct TokenLimitPauser<'info> {
    /// The account with the authority to execute the pause operation
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `PAUSER_ROLE_TOKEN_LIMIT` role
    /// # PDA Seeds
    /// - `PAUSER_ROLE_TOKEN_LIMIT`
    /// - The authority's address
    #[account(
        seeds = [RoleType::PAUSER_ROLE_TOKEN_LIMIT, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `TokenLimit` account for the specific token to pause
    /// # PDA Seeds
    /// - `TOKEN_LIMIT_ACCOUNT_SEED`
    /// - Mint address
    #[account(
        mut,
        seeds = [TOKEN_LIMIT_ACCOUNT_SEED, token_limit_account.mint.as_ref()],
        bump = token_limit_account.bump,
    )]
    pub token_limit_account: Account<'info, TokenLimit>,
}

impl<'info> TokenLimitPauser<'info> {
    /// Set `redemption_paused` to true and emit `TokenRedemptionPaused` for this mint.
    pub fn pause_token_redemption(&mut self) -> Result<()> {
        // Set the redemption_paused flag to true
        self.token_limit_account.redemption_paused = true;

        emit!(TokenRedemptionPaused {
            is_paused: true,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    /// Set `minting_paused` to true and emit `TokenMintingPaused` for this mint.
    pub fn pause_token_minting(&mut self) -> Result<()> {
        self.token_limit_account.minting_paused = true;

        emit!(TokenMintingPaused {
            is_paused: true,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }
}
