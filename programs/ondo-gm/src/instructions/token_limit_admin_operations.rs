use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::TOKEN_LIMIT_ACCOUNT_SEED,
    errors::OndoError,
    events::RateLimitTokenSet,
    state::{RoleType, Roles, TokenLimit},
};

/// Initialize a `TokenLimit` account for a GM Token/USDon
/// Requires `DEPLOYER_ROLE_GMTOKEN_FACTORY` role
#[derive(Accounts)]
pub struct InitializeTokenLimit<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to initialize token limit accounts
    pub authority: Signer<'info>,

    /// The GM Token or USDon mint
    pub mint: InterfaceAccount<'info, Mint>,

    /// The `TokenLimit` account to be initialized
    /// # PDA Seeds
    /// - `TOKEN_LIMIT_ACCOUNT_SEED`
    /// - Mint address
    #[account(
        init,
        payer = payer,
        space = 8 + TokenLimit::INIT_SPACE,
        seeds = [TOKEN_LIMIT_ACCOUNT_SEED, mint.key().as_ref()],
        bump
    )]
    pub token_limit: Account<'info, TokenLimit>,

    /// The `Roles` account verifying the authority has the `DEPLOYER_ROLE_GMTOKEN_FACTORY` role
    /// # PDA Seeds
    /// - `DEPLOYER_ROLE_GMTOKEN_FACTORY`
    /// - The authority's address
    #[account(
        seeds = [RoleType::DEPLOYER_ROLE_GMTOKEN_FACTORY, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeTokenLimit<'info> {
    /// Initialize a `TokenLimit` account for a GM Token/USDon
    /// # Arguments
    /// * `rate_limit` - The global rate limit (maximum tokens per window), if any
    /// * `limit_window` - The time window for the rate limit in seconds, if any
    /// * `default_user_rate_limit` - The default per-user rate limit, if any
    /// * `default_user_limit_window` - The default per-user time window in seconds, if any
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the TokenLimit account is successfully initialized, Err otherwise
    pub fn initialize_token_limit(
        &mut self,
        rate_limit: Option<u64>,
        limit_window: Option<u64>,
        default_user_rate_limit: Option<u64>,
        default_user_limit_window: Option<u64>,
        bumps: &InitializeTokenLimitBumps,
    ) -> Result<()> {
        // Validate limit_window is not zero if provided
        if let Some(window) = limit_window {
            require_gt!(window, 0, OndoError::InvalidRateLimit);
        }

        // Validate default_user_limit_window is not zero if provided
        if let Some(window) = default_user_limit_window {
            require_gt!(window, 0, OndoError::InvalidRateLimit);
        }

        // Initialize rate_used fields to Some(0) if rate limits are set
        let (mint_capacity_used, redeem_capacity_used) =
            if rate_limit.is_some() && limit_window.is_some() {
                (Some(0), Some(0))
            } else {
                (None, None)
            };

        // Write token limit data to the TokenLimit account
        self.token_limit.set_inner(TokenLimit {
            mint: self.mint.key(),
            rate_limit,
            limit_window,
            mint_capacity_used,
            mint_last_updated: None,
            redeem_capacity_used,
            redeem_last_updated: None,
            minting_paused: false,    // Assuming mint is not paused by default
            redemption_paused: false, // Assuming redemption is not paused by default
            default_user_rate_limit,
            default_user_limit_window,
            bump: bumps.token_limit,
        });

        // Emit event for token limit initialization
        emit!(RateLimitTokenSet {
            token: self.mint.key(),
            limit: self.token_limit.rate_limit,
            limit_window: self.token_limit.limit_window,
        });

        Ok(())
    }
}

/// Set or update the token limit parameters for a GM Token/USDon
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` role
/// Allows updating any combination of the four parameters
#[derive(Accounts)]
pub struct SetTokenLimit<'info> {
    /// The account with the authority to update token limits
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The GM Token or USDon mint
    pub mint: InterfaceAccount<'info, Mint>,

    /// The `TokenLimit` account to be updated
    /// # PDA Seeds
    /// - `TOKEN_LIMIT_ACCOUNT_SEED`
    /// - Mint address
    #[account(
        mut,
        seeds = [TOKEN_LIMIT_ACCOUNT_SEED, mint.key().as_ref()],
        bump = token_limit.bump,
    )]
    pub token_limit: Account<'info, TokenLimit>,

    /// The Roles account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,
}

impl<'info> SetTokenLimit<'info> {
    /// Set or update the token limit parameters for a GM Token/USDon
    /// Allows updating any combination of the four parameters
    /// # Arguments
    /// * `rate_limit` - The new global rate limit (maximum tokens per window), if provided
    /// * `limit_window` - The new time window for the rate limit in seconds, if provided
    /// * `default_user_rate_limit` - The new default per-user rate limit, if provided
    /// * `default_user_limit_window` - The new default per-user time window in seconds, if provided
    /// # Returns
    /// * `Result<()>` - Ok if the TokenLimit account is successfully updated, Err otherwise
    pub fn set_token_limit(
        &mut self,
        rate_limit: Option<u64>,
        limit_window: Option<u64>,
        default_user_rate_limit: Option<u64>,
        default_user_limit_window: Option<u64>,
    ) -> Result<()> {
        // Validate limit_window is not zero if provided
        if let Some(window) = limit_window {
            require_gt!(window, 0, OndoError::InvalidRateLimit);
        }

        // Validate default_user_limit_window is not zero if provided
        if let Some(window) = default_user_limit_window {
            require_gt!(window, 0, OndoError::InvalidRateLimit);
        }

        // Update rate limit fields if provided
        if let Some(new_rate_limit) = rate_limit {
            self.token_limit.rate_limit = Some(new_rate_limit);
        }

        // Update limit window if provided
        if let Some(new_limit_window) = limit_window {
            self.token_limit.limit_window = Some(new_limit_window);
        }

        // Update default user rate limit if provided
        if let Some(new_default_user_rate_limit) = default_user_rate_limit {
            self.token_limit.default_user_rate_limit = Some(new_default_user_rate_limit);
        }

        // Update default user limit window if provided
        if let Some(new_default_user_limit_window) = default_user_limit_window {
            self.token_limit.default_user_limit_window = Some(new_default_user_limit_window);
        }

        // Initialize rate_used fields if they were previously None but limits are now set
        if self.token_limit.rate_limit.is_some() && self.token_limit.limit_window.is_some() {
            if self.token_limit.mint_capacity_used.is_none() {
                self.token_limit.mint_capacity_used = Some(0);
            }
            if self.token_limit.redeem_capacity_used.is_none() {
                self.token_limit.redeem_capacity_used = Some(0);
            }
        }

        // Emit event for token limit update
        emit!(RateLimitTokenSet {
            token: self.mint.key(),
            limit: self.token_limit.rate_limit,
            limit_window: self.token_limit.limit_window,
        });

        Ok(())
    }
}
