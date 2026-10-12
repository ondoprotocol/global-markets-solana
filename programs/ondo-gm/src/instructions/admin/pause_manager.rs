use anchor_lang::prelude::*;

use crate::{
    constants::{GMTOKEN_MANAGER_STATE_SEED, PORTFOLIO_TOKEN_MANAGER_STATE_SEED},
    events::{TokenManagerMintingPaused, TokenManagerRedemptionPaused},
    instructions::TokenKind,
    state::{GMTokenManagerState, PortfolioTokenManagerState, RoleType, Roles},
};

/// Pause or resume global GM minting/redemption using `AdminRoleGMTokenManager`.
/// Changes manager flags; per-token and Token-2022 mint pauses remain independent.
#[derive(Accounts)]
pub struct GMTokenManagerAdminPauser<'info> {
    /// Administrator authorizing the global pause or resume.
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `GMTokenManagerState` account to be modified
    /// # PDA Seeds
    /// - `GMTOKEN_MANAGER_STATE_SEED`
    #[account(
        mut,
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
    )]
    pub gmtoken_manager_state: Account<'info, GMTokenManagerState>,
}

impl<'info> GMTokenManagerAdminPauser<'info> {
    /// Set `redemption_paused` to true and emit `TokenManagerRedemptionPaused`.
    pub fn pause_global_redemption(&mut self) -> Result<()> {
        self.gmtoken_manager_state.redemption_paused = true;

        emit!(TokenManagerRedemptionPaused {
            is_paused: true,
            pauser: self.authority.key(),
            kind: TokenKind::GM as u8,
        });

        Ok(())
    }

    /// Clear `redemption_paused` and emit `TokenManagerRedemptionPaused` with `is_paused = false`.
    pub fn resume_global_redemption(&mut self) -> Result<()> {
        self.gmtoken_manager_state.redemption_paused = false;

        emit!(TokenManagerRedemptionPaused {
            is_paused: false,
            pauser: self.authority.key(),
            kind: TokenKind::GM as u8,
        });

        Ok(())
    }

    /// Set `minting_paused` to true and emit `TokenManagerMintingPaused`.
    pub fn pause_global_minting(&mut self) -> Result<()> {
        self.gmtoken_manager_state.minting_paused = true;

        emit!(TokenManagerMintingPaused {
            is_paused: true,
            pauser: self.authority.key(),
            kind: TokenKind::GM as u8,
        });

        Ok(())
    }

    /// Clear `minting_paused` and emit `TokenManagerMintingPaused` with `is_paused = false`.
    pub fn resume_global_minting(&mut self) -> Result<()> {
        self.gmtoken_manager_state.minting_paused = false;

        emit!(TokenManagerMintingPaused {
            is_paused: false,
            pauser: self.authority.key(),
            kind: TokenKind::GM as u8,
        });

        Ok(())
    }
}

/// Pause global GM minting/redemption using `PauserRoleGMTokenManager`.
/// This role cannot clear pause flags; resuming requires `AdminRoleGMTokenManager`.
#[derive(Accounts)]
pub struct GMTokenManagerGlobalPauser<'info> {
    /// The account with the authority to execute the pause operation
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `PAUSER_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `PAUSER_ROLE_GMTOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::PAUSER_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `GMTokenManagerState` account to be modified
    /// # PDA Seeds
    /// - `GMTOKEN_MANAGER_STATE_SEED`
    #[account(
        mut,
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
    )]
    pub gmtoken_manager_state: Account<'info, GMTokenManagerState>,
}

impl<'info> GMTokenManagerGlobalPauser<'info> {
    /// Set `redemption_paused` to true and emit `TokenManagerRedemptionPaused`.
    pub fn pause_global_redemption(&mut self) -> Result<()> {
        self.gmtoken_manager_state.redemption_paused = true;

        emit!(TokenManagerRedemptionPaused {
            is_paused: true,
            pauser: self.authority.key(),
            kind: TokenKind::GM as u8,
        });

        Ok(())
    }
    /// Set `minting_paused` to true and emit `TokenManagerMintingPaused`.
    pub fn pause_global_minting(&mut self) -> Result<()> {
        self.gmtoken_manager_state.minting_paused = true;

        emit!(TokenManagerMintingPaused {
            is_paused: true,
            pauser: self.authority.key(),
            kind: TokenKind::GM as u8,
        });

        Ok(())
    }
}

// Pause Minting/Redemption for all Portfolio Tokens
#[derive(Accounts)]
pub struct PortfolioTokenManagerGlobalPauser<'info> {
    /// The account with the authority to execute the pause operation
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `PAUSER_ROLE_PORTFOLIO_TOKEN_MANAGER` role
    #[account(
        seeds = [RoleType::PAUSER_ROLE_PORTFOLIO_TOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    #[account(
        mut,
        seeds = [PORTFOLIO_TOKEN_MANAGER_STATE_SEED],
        bump = portfolio_token_manager.bump,
    )]
    pub portfolio_token_manager: Account<'info, PortfolioTokenManagerState>,
}

impl<'info> PortfolioTokenManagerGlobalPauser<'info> {
    /// Pause redemptions globally for all Portfolio Tokens
    pub fn pause_global_redemption(&mut self) -> Result<()> {
        self.portfolio_token_manager.redemption_paused = true;

        emit!(TokenManagerRedemptionPaused {
            is_paused: true,
            pauser: self.authority.key(),
            kind: TokenKind::Portfolio as u8,
        });

        Ok(())
    }

    /// Pause minting globally for all Portfolio Tokens
    pub fn pause_global_minting(&mut self) -> Result<()> {
        self.portfolio_token_manager.minting_paused = true;

        emit!(TokenManagerMintingPaused {
            is_paused: true,
            pauser: self.authority.key(),
            kind: TokenKind::Portfolio as u8,
        });
        Ok(())
    }
}

/// Pause/Unpause subscriptions/redemptions for all Portfolio Tokens
/// Requires `ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER` role
#[derive(Accounts)]
pub struct PortfolioTokenManagerAdminPauser<'info> {
    /// The account with the authority to execute the unpause/configuration operation
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER` role
    #[account(
        seeds = [RoleType::ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    #[account(
        mut,
        seeds = [PORTFOLIO_TOKEN_MANAGER_STATE_SEED],
        bump = portfolio_token_manager.bump,
    )]
    pub portfolio_token_manager: Account<'info, PortfolioTokenManagerState>,
}

impl<'info> PortfolioTokenManagerAdminPauser<'info> {
    /// Set `redemption_paused` to true and emit `TokenManagerRedemptionPaused`.
    pub fn pause_global_redemption(&mut self) -> Result<()> {
        self.portfolio_token_manager.redemption_paused = true;

        emit!(TokenManagerRedemptionPaused {
            is_paused: true,
            pauser: self.authority.key(),
            kind: TokenKind::Portfolio as u8,
        });

        Ok(())
    }

    /// Clear `redemption_paused` and emit `TokenManagerRedemptionPaused` with `is_paused = false`.
    pub fn resume_global_redemption(&mut self) -> Result<()> {
        self.portfolio_token_manager.redemption_paused = false;

        emit!(TokenManagerRedemptionPaused {
            is_paused: false,
            pauser: self.authority.key(),
            kind: TokenKind::Portfolio as u8,
        });

        Ok(())
    }

    /// Set `minting_paused` to true and emit `TokenManagerMintingPaused`.
    pub fn pause_global_minting(&mut self) -> Result<()> {
        self.portfolio_token_manager.minting_paused = true;

        emit!(TokenManagerMintingPaused {
            is_paused: true,
            pauser: self.authority.key(),
            kind: TokenKind::Portfolio as u8,
        });

        Ok(())
    }

    /// Clear `minting_paused` and emit `TokenManagerMintingPaused` with `is_paused = false`.
    pub fn resume_global_minting(&mut self) -> Result<()> {
        self.portfolio_token_manager.minting_paused = false;

        emit!(TokenManagerMintingPaused {
            is_paused: false,
            pauser: self.authority.key(),
            kind: TokenKind::Portfolio as u8,
        });

        Ok(())
    }
}
