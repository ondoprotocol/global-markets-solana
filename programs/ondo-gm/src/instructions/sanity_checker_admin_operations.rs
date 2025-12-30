use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::{BASIS_POINTS_DIVISOR, MAX_SECONDS_EXPIRATION, ORACLE_SANITY_CHECK_SEED},
    errors::OndoError,
    events::{RoleGranted, RoleRevoked, SanityCheckSet, SanityCheckUpdated},
    state::{OracleSanityCheck, RoleType, Roles},
};

/// Initialize an OracleSanityCheck state account for a given mint
/// Requires ADMIN_ROLE_ONDO_SANITY_CHECK role
#[derive(Accounts)]
pub struct InitializeSanityCheck<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to initialize the sanity check
    pub authority: Signer<'info>,

    /// The sanity check account to be initialized
    /// # PDA Seeds
    /// - `ORACLE_SANITY_CHECK_SEED`
    /// - `mint` (the mint address of the GM Token for which the sanity check is being initialized)
    #[account(
        init,
        payer = payer,
        space = 8 + OracleSanityCheck::INIT_SPACE,
        seeds = [ORACLE_SANITY_CHECK_SEED, mint.key().as_ref()],
        bump
    )]
    pub sanity_check: Account<'info, OracleSanityCheck>,

    /// The Roles account verifying the authority has the `ADMIN_ROLE_ONDO_SANITY_CHECK` role
    /// # PDA Seeds
    /// - `RoleType::ADMIN_ROLE_ONDO_SANITY_CHECK`
    /// - `authority` (the authority's address)
    #[account(
        seeds = [RoleType::ADMIN_ROLE_ONDO_SANITY_CHECK, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The GM Token mint for which the sanity check is being initialized
    pub mint: InterfaceAccount<'info, Mint>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeSanityCheck<'info> {
    /// Initializes the sanity check parameters
    /// # Arguments
    /// - `last_price`: The last known price of the GM Token
    /// - `allowed_deviation_bps`: The allowed percentage deviation in basis points
    /// - `max_time_delay`: The maximum time delay for price validity in seconds
    /// - `bumps`: Bumps for PDA derivation
    /// # Returns
    /// * `Result<()>` - Result indicating success or failure
    pub fn initialize_sanity_check(
        &mut self,
        last_price: u64,
        allowed_deviation_bps: u64,
        max_time_delay: i64,
        bumps: &InitializeSanityCheckBumps,
    ) -> Result<()> {
        // Validate allowed deviation
        require!(
            allowed_deviation_bps <= BASIS_POINTS_DIVISOR, // 100% in basis points, to be adjusted
            OndoError::InvalidPercentage
        );

        // Validate price delay
        require!(
            max_time_delay <= MAX_SECONDS_EXPIRATION, // 1 year lifetime in days, to be adjusted
            OndoError::InvalidMaxTimeDelay
        );

        require_gt!(last_price, 0, OndoError::InvalidPrice);

        // Write to the sanity check account
        self.sanity_check.set_inner(OracleSanityCheck {
            last_price,
            mint: self.mint.key(),
            allowed_deviation_bps,
            max_time_delay,
            price_last_updated: Clock::get()?.unix_timestamp,
            bump: bumps.sanity_check,
        });

        // Emit event
        emit!(SanityCheckSet {
            mint: self.mint.key(),
            allowed_deviation_bps,
            max_time_delay,
        });

        Ok(())
    }
}

/// Grant a Sanity Check Setter Role to a user by initializing their `Roles` account
/// Requires `ADMIN_ROLE_ONDO_SANITY_CHECK` role
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct OndoSanitySetterGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant the setter role
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_ONDO_SANITY_CHECK` role
    /// # PDA Seeds
    /// - `RoleType::ADMIN_ROLE_ONDO_SANITY_CHECK`
    /// - `authority` (the authority's address)
    #[account(
        seeds = [RoleType::ADMIN_ROLE_ONDO_SANITY_CHECK, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The role account to be initialized for the user
    /// # PDA Seeds
    /// - `role.seed()` (the seed for the setter role)
    /// - `user` (the user's address)
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

impl<'info> OndoSanitySetterGrantRole<'info> {
    /// Adds the setter role to the specified user
    /// # Arguments
    /// - `role`: The RoleType to assign (must be SetterRoleOndoSanityCheck)
    /// - `user`: The Pubkey of the user to whom the role is being assigned
    /// - `bumps`: Bumps for PDA derivation
    /// # Returns
    /// * `Result<()>` - Result indicating success or failure
    pub fn grant_setter_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &OndoSanitySetterGrantRoleBumps,
    ) -> Result<()> {
        // Ensure the role is SetterRoleOndoSanityCheck
        require!(
            matches!(role, RoleType::SetterRoleOndoSanityCheck),
            OndoError::InvalidRoleType
        );

        // Write to the roles destination account
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

/// Revoke a Sanity Check Setter Role from a user by closing their `Roles` account
/// Requires `ADMIN_ROLE_ONDO_SANITY_CHECK` role
#[derive(Accounts)]
pub struct OndoSanitySetterRevokeRole<'info> {
    /// Receives the lamports from closing the Roles account
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The account that has the authority to revoke the setter role
    pub authority: Signer<'info>,

    /// The role account of the admin
    /// # PDA Seeds
    /// - `RoleType::ADMIN_ROLE_ONDO_SANITY_CHECK`
    /// - `authority` (the authority's address)
    #[account(
        seeds = [RoleType::ADMIN_ROLE_ONDO_SANITY_CHECK, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The role account to be closed
    /// # PDA Seeds
    /// - `role_to_revoke.role.seed()` (the seed for the setter role)
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

impl<'info> OndoSanitySetterRevokeRole<'info> {
    /// Revokes the setter role from the specified user
    /// # Returns
    /// * `Result<()>` - Result indicating success or failure
    pub fn revoke_setter_role(&mut self) -> Result<()> {
        // Ensure the role is SetterRoleOndoSanityCheck
        require!(
            matches!(
                self.role_to_revoke.role,
                RoleType::SetterRoleOndoSanityCheck
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

/// Set the last price in the `SanityCheck` state account
/// Requires `SETTER_ROLE_ONDO_SANITY_CHECK` role
#[derive(Accounts)]
pub struct SetSanityCheck<'info> {
    /// The account with the authority to set the last price
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `SETTER_ROLE_ONDO_SANITY_CHECK` role
    /// # PDA Seeds
    /// - `SETTER_ROLE_ONDO_SANITY_CHECK`
    /// - The authority's address
    #[account(
        seeds = [RoleType::SETTER_ROLE_ONDO_SANITY_CHECK, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The GM Token mint
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// The `OracleSanityCheck` account to be updated
    /// # PDA Seeds
    /// - `ORACLE_SANITY_CHECK_SEED`
    /// - Mint address
    #[account(
        mut,
        seeds = [ORACLE_SANITY_CHECK_SEED, mint.key().as_ref()],
        bump = sanity_check_account.bump,
    )]
    pub sanity_check_account: Box<Account<'info, OracleSanityCheck>>,
}

impl<'info> SetSanityCheck<'info> {
    /// Set the last price in the sanity check
    /// # Arguments
    /// * `last_price` - The new last price (must be greater than 0)
    /// # Returns
    /// * `Result<()>` - Ok if the price is successfully set, Err otherwise
    pub fn set_last_price(&mut self, last_price: u64) -> Result<()> {
        require!(last_price > 0, OndoError::InvalidPrice);

        self.sanity_check_account.last_price = last_price;
        self.sanity_check_account.price_last_updated = Clock::get()?.unix_timestamp;

        emit!(SanityCheckUpdated {
            mint: self.mint.key(),
            last_price: Some(last_price),
            allowed_deviation_bps: None,
            max_time_delay: None,
        });

        Ok(())
    }
}

/// Grant a Sanity Check Configurer Role to a user by initializing their `Roles` account
/// Requires `ADMIN_ROLE_ONDO_SANITY_CHECK` role
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct OndoSanityConfigurerGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant the configurer role
    pub authority: Signer<'info>,

    /// The Roles account verifying the authority has the `ADMIN_ROLE_ONDO_SANITY_CHECK` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_ONDO_SANITY_CHECK`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_ONDO_SANITY_CHECK, authority.key().as_ref()],
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

impl<'info> OndoSanityConfigurerGrantRole<'info> {
    /// Grant a configurer role to a user
    /// # Arguments
    /// * `role` - The role to grant (must be ConfigurerRoleOndoSanityCheck)
    /// * `user` - The public key of the user to grant the role to
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully granted, Err otherwise
    pub fn grant_configurer_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &OndoSanityConfigurerGrantRoleBumps,
    ) -> Result<()> {
        // Ensure only ConfigurerRoleOndoSanityCheck role can be created
        require!(
            matches!(role, RoleType::ConfigurerRoleOndoSanityCheck),
            OndoError::InvalidRoleType
        );

        // Write state
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

/// Revoke a Sanity Check Configurer Role from a user by closing their `Roles` account
/// Requires `ADMIN_ROLE_ONDO_SANITY_CHECK` role
#[derive(Accounts)]
pub struct OndoSanityConfigurerRevokeRole<'info> {
    /// Receives the lamports from closing the Roles account
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The account with the authority to revoke the configurer role
    pub authority: Signer<'info>,

    /// The Roles account verifying the authority has the `ADMIN_ROLE_ONDO_SANITY_CHECK` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_ONDO_SANITY_CHECK`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_ONDO_SANITY_CHECK, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The Roles account being closed
    /// # PDA Seeds
    /// - `role_to_revoke.role.seed()` (the seed for the configurer role)
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

impl<'info> OndoSanityConfigurerRevokeRole<'info> {
    /// Revoke the configurer role from a user
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully revoked, Err otherwise
    pub fn revoke_configurer_role(&mut self) -> Result<()> {
        // Ensure only ConfigurerRoleOndoSanityCheck role can be revoked
        require!(
            matches!(
                self.role_to_revoke.role,
                RoleType::ConfigurerRoleOndoSanityCheck
            ),
            OndoError::InvalidRoleType
        );

        // Emit event for role revocation
        emit!(RoleRevoked {
            role: self.role_to_revoke.role,
            grantee: self.role_to_revoke.address,
            revoker: self.authority.key(),
        });

        Ok(())
    }
}

/// Configure the sanity check parameters for a GM token
/// Requires `CONFIGURER_ROLE_ONDO_SANITY_CHECK` role
/// Parameters that can be configured:
/// - allowed_deviation_bps: the allowed percentage deviation in basis points
/// - max_time_delay: the lifetime of the price in days
#[derive(Accounts)]
pub struct ConfigSanityCheck<'info> {
    /// The account with the authority to configure the sanity check
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `CONFIGURER_ROLE_ONDO_SANITY_CHECK` role
    /// # PDA Seeds
    /// - `CONFIGURER_ROLE_ONDO_SANITY_CHECK`
    /// - The authority's address
    #[account(
        seeds = [RoleType::CONFIGURER_ROLE_ONDO_SANITY_CHECK, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The GM Token mint
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// The OracleSanityCheck account to be configured
    /// # PDA Seeds
    /// - `ORACLE_SANITY_CHECK_SEED`
    /// - Mint address
    #[account(
        mut,
        seeds = [ORACLE_SANITY_CHECK_SEED, mint.key().as_ref()],
        bump = sanity_check_account.bump,
    )]
    pub sanity_check_account: Box<Account<'info, OracleSanityCheck>>,
}

impl<'info> ConfigSanityCheck<'info> {
    /// Set the allowed deviation in basis points
    /// # Arguments
    /// * `allowed_deviation_bps` - The allowed percentage deviation in basis points (max 10,000 = 100%)
    /// # Returns
    /// * `Result<()>` - Ok if the deviation is successfully set, Err otherwise
    pub fn set_allowed_deviation_bps(&mut self, allowed_deviation_bps: u64) -> Result<()> {
        // Validate allowed deviation
        require!(
            allowed_deviation_bps <= BASIS_POINTS_DIVISOR,
            OndoError::InvalidPercentage
        );

        // Write state
        self.sanity_check_account.allowed_deviation_bps = allowed_deviation_bps;

        // Emit event
        emit!(SanityCheckUpdated {
            mint: self.mint.key(),
            last_price: None,
            allowed_deviation_bps: Some(allowed_deviation_bps),
            max_time_delay: None,
        });

        Ok(())
    }

    /// Set the maximum time delay for price validity
    /// # Arguments
    /// * `max_time_delay` - The maximum time delay in seconds (max 1 year)
    /// # Returns
    /// * `Result<()>` - Ok if the time delay is successfully set, Err otherwise
    pub fn set_max_time_delay(&mut self, max_time_delay: i64) -> Result<()> {
        // Validate max time delay
        require!(
            max_time_delay <= MAX_SECONDS_EXPIRATION,
            OndoError::InvalidMaxTimeDelay
        );

        // Write state
        self.sanity_check_account.max_time_delay = max_time_delay;

        // Emit event
        emit!(SanityCheckUpdated {
            mint: self.mint.key(),
            last_price: None,
            allowed_deviation_bps: None,
            max_time_delay: Some(max_time_delay),
        });

        Ok(())
    }
}
