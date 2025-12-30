use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::*,
    errors::OndoError,
    events::{
        GMTokenMintingPaused, GMTokenRedemptionPaused, RateLimitUserSet, RoleGranted, RoleRevoked,
        SetTradingHoursOffset, TokenManagerMintingPaused, TokenManagerRedemptionPaused,
    },
    state::{GMTokenManagerState, OndoUser, RoleType, Roles, TokenLimit},
};

/// Initialize the `GmTokenManagerState` account
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` role
#[derive(Accounts)]
pub struct InitializeGMTokenManager<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to initialize the GM Token Manager
    pub authority: Signer<'info>,

    /// The `GmTokenManagerState` account to be initialized
    /// # PDA Seeds
    /// - `GMTOKEN_MANAGER_STATE_SEED`
    #[account(
        init,
        payer = payer,
        space = 8 + GMTokenManagerState::INIT_SPACE,
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump
    )]
    pub gmtoken_manager_state: Account<'info, GMTokenManagerState>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeGMTokenManager<'info> {
    /// Initialize the `GmTokenManagerState` account
    /// # Arguments
    /// * `factory_paused` - Whether the GM Token factory should start in a paused state
    /// * `redemptions_paused` - Whether redemptions should start in a paused state
    /// * `subscriptions_paused` - Whether subscriptions should start in a paused state
    /// * `attestation_signer_secp` - The secp256k1 Ethereum address of the attestation signer (20 bytes)
    /// * `trading_hours_offset` - The trading offset in seconds from UTC for trading hours
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the GmTokenManagerState is successfully initialized, Err otherwise
    pub fn initialize_gmtoken_manager(
        &mut self,
        factory_paused: bool,
        redemption_paused: bool,
        minting_paused: bool,
        attestation_signer_secp: [u8; 20],
        trading_hours_offset: i64,
        bumps: &InitializeGMTokenManagerBumps,
    ) -> Result<()> {
        // Validate trading hours offset
        self.gmtoken_manager_state
            .validate_trading_hours_offset(trading_hours_offset)?;

        self.gmtoken_manager_state.set_inner(GMTokenManagerState {
            execution_id: None,
            factory_paused,
            redemption_paused,
            minting_paused,
            bump: bumps.gmtoken_manager_state,
            attestation_signer_secp,
            trading_hours_offset,
        });

        Ok(())
    }
}

/// Grant a GM Token Manager role to a user by initializing a `Roles` account
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` role
/// Only the `PauserRoleGmtokenManager` or `IssuanceHoursRole` roles can be added
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct GMTokenManagerGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant GM Token Manager roles
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - ADMIN_ROLE_GMTOKEN_MANAGER
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
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

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> GMTokenManagerGrantRole<'info> {
    /// Add a GM Token Manager role to a user
    /// # Arguments
    /// * `role` - The role to grant (must be `PauserRoleGmtokenManager` or `IssuanceHoursRole`)
    /// * `user` - The public key of the user to grant the role to
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully granted, Err otherwise
    pub fn add_gmtoken_manager_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &GMTokenManagerGrantRoleBumps,
    ) -> Result<()> {
        // Only allow PauserRoleGmtokenManager and IssuanceHoursRole roles to be created
        require!(
            matches!(
                role,
                RoleType::PauserRoleGMTokenManager | RoleType::IssuanceHoursRole
            ),
            OndoError::InvalidRoleType
        );

        // Write to the new Roles account
        self.role_to_grant.address = user;
        self.role_to_grant.role = role;
        self.role_to_grant.bump = bumps.role_to_grant;

        // Emit event for role granted
        emit!(RoleGranted {
            role,
            grantee: user,
            granter: self.authority.key(),
        });

        Ok(())
    }
}

/// Revoke a GM Token Manager role from a user by closing their `Roles` account
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` role
/// Only the `PauserRoleGmtokenManager` and `IssuanceHoursRole` roles can be removed
#[derive(Accounts)]
pub struct GMTokenManagerRevokeRole<'info> {
    /// The account with the authority to revoke GM Token Manager roles
    pub authority: Signer<'info>,

    /// Receives the lamports from closing the Roles account
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The Roles account being closed
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
        bump = role_to_revoke.bump,
    )]
    pub role_to_revoke: Account<'info, Roles>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> GMTokenManagerRevokeRole<'info> {
    /// Revoke a GM Token Manager role from a user by closing their `Roles` account
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully revoked, Err otherwise
    pub fn revoke_gmtoken_manager_role(&mut self) -> Result<()> {
        // Only allow PauserRoleGmtokenManager or IssuanceHoursRole roles to be revoked
        require!(
            matches!(
                self.role_to_revoke.role,
                RoleType::PauserRoleGMTokenManager | RoleType::IssuanceHoursRole
            ),
            OndoError::InvalidRoleType
        );

        // Emit event for role revoked
        emit!(RoleRevoked {
            role: self.role_to_revoke.role,
            grantee: self.role_to_revoke.address,
            revoker: self.authority.key(),
        });

        Ok(())
    }
}

// Pause Minting/Redemption for all GM Tokens
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

    /// The `GmTokenManagerState` account to be modified
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
    /// Pause redemptions globally for all GM Tokens
    /// # Returns
    /// * `Result<()>` - Ok if redemptions are successfully paused, Err otherwise
    pub fn pause_global_redemption(&mut self) -> Result<()> {
        // Set the redemption_paused flag to true
        self.gmtoken_manager_state.redemption_paused = true;

        // Emit event for redemptions paused
        emit!(TokenManagerRedemptionPaused {
            is_paused: true,
            pauser: self.authority.key(),
        });

        Ok(())
    }
    /// Pause minting globally for all GM Tokens
    /// # Returns
    /// * `Result<()>` - Ok if subscriptions are successfully paused, Err otherwise
    pub fn pause_global_minting(&mut self) -> Result<()> {
        self.gmtoken_manager_state.minting_paused = true;

        emit!(TokenManagerMintingPaused {
            is_paused: true,
            pauser: self.authority.key(),
        });
        Ok(())
    }
}

// Pause Subscription/Redemption for a GM Token
#[derive(Accounts)]
pub struct GMTokenManagerTokenPauser<'info> {
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

    /// The `TokenLimit` account for the specific GM Token to pause
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

impl<'info> GMTokenManagerTokenPauser<'info> {
    /// Pause redemptions for a specific GM Token
    /// # Returns
    /// * `Result<()>` - Ok if redemptions are successfully paused, Err otherwise
    pub fn pause_gmtoken_redemption(&mut self) -> Result<()> {
        // Set the redemption_paused flag to true
        self.token_limit_account.redemption_paused = true;

        // Emit event for redemptions paused
        emit!(GMTokenRedemptionPaused {
            is_paused: true,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    /// Pause for minting a specific GM Token
    /// # Returns
    /// * `Result<()>` - Ok if subscriptions are successfully paused, Err otherwise
    pub fn pause_gmtoken_minting(&mut self) -> Result<()> {
        self.token_limit_account.minting_paused = true;

        // Emit event for minting paused
        emit!(GMTokenMintingPaused {
            is_paused: true,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }
}

/// Pause/Unpause subscriptions/redemptions for all GM Tokens
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` role
/// Can also set attestation signer
#[derive(Accounts)]
pub struct GMTokenManagerAdminGlobalPauser<'info> {
    /// The account with the authority to execute the unpause/configuration operation
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

    /// The `GmTokenManagerState` account to be modified
    /// # PDA Seeds
    /// - `GMTOKEN_MANAGER_STATE_SEED`
    #[account(
        mut,
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
    )]
    pub gmtoken_manager_state: Account<'info, GMTokenManagerState>,
}

impl<'info> GMTokenManagerAdminGlobalPauser<'info> {
    pub fn pause_global_redemption(&mut self) -> Result<()> {
        self.gmtoken_manager_state.redemption_paused = true;

        // Emit event for redemptions pause state change
        emit!(TokenManagerRedemptionPaused {
            is_paused: true,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    pub fn resume_global_redemption(&mut self) -> Result<()> {
        self.gmtoken_manager_state.redemption_paused = false;

        emit!(TokenManagerRedemptionPaused {
            is_paused: false,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    pub fn pause_global_minting(&mut self) -> Result<()> {
        self.gmtoken_manager_state.minting_paused = true;

        emit!(TokenManagerMintingPaused {
            is_paused: true,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    pub fn resume_global_minting(&mut self) -> Result<()> {
        self.gmtoken_manager_state.minting_paused = false;

        emit!(TokenManagerMintingPaused {
            is_paused: false,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    /// Set the attestation signer secp256k1 Ethereum address
    /// # Arguments
    /// * `attestation_signer_secp` - The new secp256k1 Ethereum address of the attestation signer (20 bytes)
    /// # Returns
    /// * `Result<()>` - Ok if the attestation signer is successfully updated, Err otherwise
    pub fn set_attestation_signer_secp(&mut self, attestation_signer_secp: [u8; 20]) -> Result<()> {
        // Update the attestation signer address
        self.gmtoken_manager_state.attestation_signer_secp = attestation_signer_secp;

        Ok(())
    }
}

/// Pause subscription/redemptions for a GM Token
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` role
#[derive(Accounts)]
pub struct GMTokenManagerAdminTokenPauser<'info> {
    /// The account with the authority to execute the pause/unpause operation
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

    /// The `TokenLimit` account for the specific GM Token to pause/unpause
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

impl<'info> GMTokenManagerAdminTokenPauser<'info> {
    pub fn pause_gmtoken_redemption(&mut self) -> Result<()> {
        self.token_limit_account.redemption_paused = true;

        // Emit event for redemptions pause state change
        emit!(GMTokenRedemptionPaused {
            is_paused: true,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    pub fn pause_gmtoken_minting(&mut self) -> Result<()> {
        self.token_limit_account.minting_paused = true;

        emit!(GMTokenMintingPaused {
            is_paused: true,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    pub fn resume_gmtoken_redemption(&mut self) -> Result<()> {
        self.token_limit_account.redemption_paused = false;

        emit!(GMTokenRedemptionPaused {
            is_paused: false,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    pub fn resume_gmtoken_minting(&mut self) -> Result<()> {
        self.token_limit_account.minting_paused = false;

        emit!(GMTokenMintingPaused {
            is_paused: false,
            token: self.token_limit_account.mint,
            pauser: self.authority.key(),
        });

        Ok(())
    }
}

/// Set rate limit for a user on a GM Token
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` role
#[derive(Accounts)]
pub struct GMTokenManagerAdminSetUserLimits<'info> {
    /// The account with the authority to set user limits
    pub authority: Signer<'info>,

    /// The GM Token mint
    pub mint: InterfaceAccount<'info, Mint>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `OndoUser` account to update
    /// # PDA Seeds
    /// - `ONDO_USER_SEED`
    /// - User's owner address
    /// - Mint address
    #[account(
        mut,
        seeds = [ONDO_USER_SEED, ondo_user.owner.as_ref(), mint.key().as_ref()],
        bump = ondo_user.bump,
    )]
    pub ondo_user: Account<'info, OndoUser>,

    /// The TokenLimit account
    /// # PDA Seeds
    /// - TOKEN_LIMIT_ACCOUNT_SEED
    /// - Mint address
    #[account(
        seeds = [TOKEN_LIMIT_ACCOUNT_SEED, mint.key().as_ref()],
        bump = token_limit.bump,
        has_one = mint
    )]
    pub token_limit: Account<'info, TokenLimit>,
}

impl<'info> GMTokenManagerAdminSetUserLimits<'info> {
    /// Set the rate limit for a specific user on a GM Token
    /// # Arguments
    /// * `rate_limit` - The new rate limit for the user (maximum tokens per window)
    /// # Returns
    /// * `Result<()>` - Ok if the user rate limit is successfully set, Err otherwise
    pub fn set_ondo_user_limits(&mut self, rate_limit: u64, limit_window: u64) -> Result<()> {
        // Set the rate_limit field
        self.ondo_user.rate_limit = Some(rate_limit);

        // Set the limit_window field
        self.ondo_user.limit_window = Some(limit_window);

        // If limit_window is set to 0, default to token_limit's default_user_limit_window if set,
        // otherwise use DEFAULT_LIMIT_WINDOW.
        if self.ondo_user.limit_window == Some(0) {
            if self.token_limit.default_user_limit_window.is_none() {
                self.ondo_user.limit_window = Some(DEFAULT_LIMIT_WINDOW);
            } else {
                self.ondo_user.limit_window = self.token_limit.default_user_limit_window;
            }
        }

        // Initialize rate_used fields if not already set
        if self.ondo_user.mint_capacity_used.is_none() {
            self.ondo_user.mint_capacity_used = Some(0);
        }
        if self.ondo_user.redeem_capacity_used.is_none() {
            self.ondo_user.redeem_capacity_used = Some(0);
        }

        // Emit event for rate limit set
        emit!(RateLimitUserSet {
            user: self.ondo_user.owner,
            limit: rate_limit,
        });

        Ok(())
    }
}

/// Set trading hours offset for the GM token manager
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` or `ISSUANCE_HOURS_ROLE` role
#[derive(Accounts)]
pub struct GMTokenManagerAdminSetTradingHoursOffset<'info> {
    /// The account with the authority to set the trading hours offset
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_MANAGER` or `ISSUANCE_HOURS_ROLE`
    /// - The authority's address
    #[account(
        seeds = [authority_role_account.role.seed(), authority.key().as_ref()],
        bump = authority_role_account.bump,
        constraint = authority_role_account.role == RoleType::AdminRoleGMTokenManager ||
            authority_role_account.role == RoleType::IssuanceHoursRole @
            OndoError::AddressNotFoundInRole
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The GmTokenManagerState account to be modified
    #[account(
        mut,
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
    )]
    pub gmtoken_manager_state: Account<'info, GMTokenManagerState>,
}

impl<'info> GMTokenManagerAdminSetTradingHoursOffset<'info> {
    /// Set the trading hours offset
    ///
    /// For Eastern Time with 8PM Friday -> 8PM Sunday closure (markets closed on weekends):
    /// The offset shifts timestamps so that 8PM ET aligns with midnight (00:00), making
    /// Saturday/Sunday (days 5-6) fall outside valid trading hours (Monday-Friday, days 0-4).
    ///
    /// # Arguments
    /// * `new_trading_hours_offset` - The timezone offset in seconds from UTC
    ///
    /// # Returns
    /// * `Result<()>` - Success if the offset is valid and updated
    ///
    /// # Eastern Time Values
    ///
    /// **Eastern Standard Time (EST):** `new_trading_hours_offset = -3600` seconds (UTC-1)
    /// - EST is UTC-5 (-18000s) + 4-hour alignment (+14400s) = -3600s
    ///
    /// **Eastern Daylight Time (EDT):** `new_trading_hours_offset = 0` seconds (UTC+0)
    /// - EDT is UTC-4 (-14400s) + 4-hour alignment (+14400s) = 0s
    ///
    /// # Daylight Savings
    ///
    /// This offset must be manually updated when transitioning between EST and EDT
    /// (typically the second Sunday in March and the first Sunday in November).
    pub fn set_trading_hours_offset(&mut self, new_trading_hours_offset: i64) -> Result<()> {
        let prev_trading_hours_offset = self.gmtoken_manager_state.trading_hours_offset;

        // Validate the new trading hours offset
        self.gmtoken_manager_state
            .validate_trading_hours_offset(new_trading_hours_offset)?;

        // Update the trading hours offset
        self.gmtoken_manager_state.trading_hours_offset = new_trading_hours_offset;

        // Emit event for trading hours offset change
        emit!(SetTradingHoursOffset {
            prev_trading_hours_offset,
            new_trading_hours_offset
        });

        Ok(())
    }
}
