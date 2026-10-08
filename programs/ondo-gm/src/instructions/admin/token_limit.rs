use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::TOKEN_LIMIT_ACCOUNT_SEED,
    errors::OndoError,
    events::{RateLimitTokenSet, RoleGranted, RoleRevoked},
    state::{RoleType, Roles, TokenLimit},
};

/// Initialize a `TokenLimit` account for a GM Token, Portfolio Token, or USDon
/// Requires `DEPLOYER_ROLE_TOKEN_FACTORY` role
#[derive(Accounts)]
pub struct InitializeTokenLimit<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to initialize token limit accounts
    pub authority: Signer<'info>,

    /// The mint to initialize limits for. Not constrained to a token family: any mint is
    /// accepted (in practice a GM Token, Portfolio Token, or USDon)
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

    /// The `Roles` account verifying the authority has the `DEPLOYER_ROLE_TOKEN_FACTORY` role
    /// # PDA Seeds
    /// - `DEPLOYER_ROLE_TOKEN_FACTORY`
    /// - The authority's address
    #[account(
        seeds = [RoleType::DEPLOYER_ROLE_TOKEN_FACTORY, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeTokenLimit<'info> {
    /// Initialize a `TokenLimit` account for a GM Token, Portfolio Token, or USDon
    /// # Arguments
    /// * `rate_limit` - The global USD notional capacity per direction, scaled to 9 decimals, if set
    /// * `limit_window` - The time window for the rate limit in seconds, if any
    /// * `default_user_rate_limit` - The default per-user USD notional capacity per direction, scaled to 9 decimals
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

/// Grant a Token Limit role to a user by initializing a `Roles` account
/// Requires `ADMIN_ROLE_TOKEN_LIMIT` role
/// Only the `PauserRoleTokenLimit` role can be granted
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct TokenLimitGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant a Token Limit role
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_TOKEN_LIMIT` role
    #[account(
        seeds = [RoleType::ADMIN_ROLE_TOKEN_LIMIT, authority.key().as_ref()],
        bump = authority_role_account.bump
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The new `Roles` account being created for the user
    /// # PDA Seeds
    /// - Role seed (from RoleType)
    /// - User's address
    #[account(
        init,
        payer = payer,
        space = Roles::INIT_SPACE,
        seeds = [role.seed(), user.as_ref()],
        bump
    )]
    pub role_to_grant: Account<'info, Roles>,

    pub system_program: Program<'info, System>,
}

impl<'info> TokenLimitGrantRole<'info> {
    /// Grant `PauserRoleTokenLimit`
    /// # Arguments
    /// * `role` - The role to grant (must be `PauserRoleTokenLimit`)
    /// * `user` - The public key of the user to grant the role to
    /// * `bumps` - The PDA bumps for account derivation
    /// # Emits
    /// * `RoleGranted`
    pub fn grant_token_limit_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &TokenLimitGrantRoleBumps,
    ) -> Result<()> {
        require!(
            matches!(role, RoleType::PauserRoleTokenLimit),
            OndoError::InvalidRoleType,
        );

        self.role_to_grant.address = user;
        self.role_to_grant.role = role;
        self.role_to_grant.bump = bumps.role_to_grant;

        emit!(RoleGranted {
            role,
            grantee: user,
            granter: self.authority.key(),
        });

        Ok(())
    }
}

/// Revoke a Token Limit role from a user by closing their `Roles` account
/// Requires `ADMIN_ROLE_TOKEN_LIMIT` role
/// Only the `PauserRoleTokenLimit` role can be revoked
#[derive(Accounts)]
pub struct TokenLimitRevokeRole<'info> {
    /// The account with the authority to revoke Token Limit roles
    pub authority: Signer<'info>,

    /// Receives the lamports from closing the Roles account
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_TOKEN_LIMIT` role
    #[account(
        seeds = [RoleType::ADMIN_ROLE_TOKEN_LIMIT, authority.key().as_ref()],
        bump = authority_role_account.bump
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `Roles` account being closed
    /// # PDA Seeds
    /// - `role_to_revoke.role.seed()` (the seed for the role)
    /// - `role_to_revoke.address` (the user's address)
    #[account(
        mut,
        close = recipient,
        seeds = [
            role_to_revoke.role.seed(),
            role_to_revoke.address.as_ref()
        ],
        bump = role_to_revoke.bump
    )]
    pub role_to_revoke: Account<'info, Roles>,

    pub system_program: Program<'info, System>,
}

impl<'info> TokenLimitRevokeRole<'info> {
    /// Revoke the `PauserRoleTokenLimit`, refund rent to `recipient`, and emit `RoleRevoked`
    pub fn revoke_token_limit_role(&mut self) -> Result<()> {
        // Only allow the `PauserRoleTokenLimit` role to be revoked
        require!(
            matches!(self.role_to_revoke.role, RoleType::PauserRoleTokenLimit),
            OndoError::InvalidRoleType,
        );

        emit!(RoleRevoked {
            role: self.role_to_revoke.role,
            grantee: self.role_to_revoke.address,
            revoker: self.authority.key(),
        });

        Ok(())
    }
}

/// Set or update the token limit parameters for a GM Token, Portfolio Token or USDon
/// Requires `ADMIN_ROLE_TOKEN_LIMIT` role
/// Allows updating any combination of the four parameters
#[derive(Accounts)]
pub struct SetTokenLimit<'info> {
    /// The account with the authority to update token limits
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The mint whose `TokenLimit` is updated. Not constrained to a token family: any mint
    /// with a `TokenLimit` account is accepted
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

    /// The Roles account verifying the authority has the `ADMIN_ROLE_TOKEN_LIMIT` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_TOKEN_LIMIT`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_TOKEN_LIMIT, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,
}

impl<'info> SetTokenLimit<'info> {
    /// Set or update the token limit parameters for a GM Token, Portfolio Token, or USDon
    /// `None` leaves a setting unchanged; updated defaults apply to new OndoUser accounts.
    /// # Arguments
    /// * `rate_limit` - The new global USD notional capacity per direction, scaled to 9 decimals
    /// * `limit_window` - The new time window for the rate limit in seconds, if provided
    /// * `default_user_rate_limit` - The new per-user USD notional capacity, scaled to 9 decimals
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
