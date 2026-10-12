use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::{DEFAULT_LIMIT_WINDOW, ONDO_USER_SEED, TOKEN_LIMIT_ACCOUNT_SEED},
    events::RateLimitUserSet,
    state::{OndoUser, RoleType, Roles, TokenLimit},
};

/// Initialize a new `OndoUser` account for a user and mint pair with the mint's default user
/// limits. Requires `INITIALIZER_ROLE_ONDO_USER`; setting limits requires `ADMIN_ROLE_USER_LIMIT`.
#[derive(Accounts)]
pub struct InitializeUser<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to initialize an `OndoUser` account
    pub authority: Signer<'info>,

    /// The user for whom the OndoUser account is being initialized
    pub user: SystemAccount<'info>,

    /// The mint associated with the OndoUser account (not constrained to a token family)
    pub mint: InterfaceAccount<'info, Mint>,

    /// The Roles account verifying the authority has the `INITIALIZER_ROLE_ONDO_USER` role
    /// # PDA Seeds
    /// - `INITIALIZER_ROLE_ONDO_USER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::INITIALIZER_ROLE_ONDO_USER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `OndoUser` account to be initialized
    /// # PDA seeds:
    /// - `ONDO_USER_SEED`
    /// - User's address,
    /// - The mint address
    #[account(
        init,
        payer = payer,
        space = 8 + OndoUser::INIT_SPACE,
        seeds = [ONDO_USER_SEED, user.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    pub ondo_user: Account<'info, OndoUser>,

    /// The mint's `TokenLimit` account, which supplies the default user limits
    /// # PDA Seeds
    /// - TOKEN_LIMIT_ACCOUNT_SEED
    /// - Mint address
    #[account(
        seeds = [TOKEN_LIMIT_ACCOUNT_SEED, mint.key().as_ref()],
        bump = token_limit.bump,
        has_one = mint
    )]
    pub token_limit: Account<'info, TokenLimit>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeUser<'info> {
    /// Initialize the OndoUser account with the mint's `TokenLimit` default user limits.
    ///
    /// This is the same initialization a user's first trade performs, so this role cannot give
    /// a user limits other than the defaults. Custom limits go through `set_ondo_user_limits`.
    /// # Arguments
    /// * `bumps` - Bumps for PDA derivation.
    /// # Returns
    /// * `Result<()>` - Result indicating success or failure.
    pub fn initialize_user(&mut self, bumps: &InitializeUserBumps) -> Result<()> {
        self.ondo_user.initialize(
            &self.user.key(),
            &self.mint.key(),
            self.token_limit.default_user_rate_limit,
            self.token_limit.default_user_limit_window,
            bumps.ondo_user,
        );

        Ok(())
    }
}

/// Set user-level minting and redemption limits for one GM or portfolio mint.
/// Requires `ADMIN_ROLE_USER_LIMIT` role
#[derive(Accounts)]
pub struct SetUserLimits<'info> {
    /// The account with the authority to set user limits
    pub authority: Signer<'info>,

    /// The GM/Portfolio token mint
    pub mint: InterfaceAccount<'info, Mint>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_USER_LIMIT` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_USER_LIMIT`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_USER_LIMIT, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `OndoUser` account to update
    /// # PDA Seeds
    /// - `ONDO_USER_SEED`
    /// - `OndoUser` owner address
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

impl<'info> SetUserLimits<'info> {
    /// Set the rate limit and limit window for a specific user on one mint
    /// # Arguments
    /// * `rate_limit` - Maximum USD notional per window, scaled to 9 decimals, per direction
    /// * `limit_window` - The new window in seconds; 0 uses the `TokenLimit` default user
    ///   window, or `DEFAULT_LIMIT_WINDOW` if that is unset
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
