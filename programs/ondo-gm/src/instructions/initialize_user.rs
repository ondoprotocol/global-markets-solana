use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::ONDO_USER_SEED,
    state::{OndoUser, RoleType, Roles},
};

/// Initialize a new `OndoUser` account for a user and mint pair.
/// Optionally sets rate limiting parameters.
#[derive(Accounts)]
pub struct InitializeUser<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to initialize an `OndoUser` account
    pub authority: Signer<'info>,

    /// The user for whom the OndoUser account is being initialized
    pub user: SystemAccount<'info>,

    /// The GM Token mint associated with the OndoUser account
    pub mint: InterfaceAccount<'info, Mint>,

    /// The Roles account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `OndoUser` account to be initialized
    /// # PDA seeds:
    /// - `ONDO_USER_SEED`
    /// - User's address,
    /// - The GM Token's mint address
    #[account(
        init,
        payer = payer,
        space = 8 + OndoUser::INIT_SPACE,
        seeds = [ONDO_USER_SEED, user.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    pub ondo_user: Account<'info, OndoUser>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeUser<'info> {
    /// Initialize the OndoUser account with optional rate limiting parameters.
    /// # Arguments
    /// * `rate_limit` - Optional maximum number of tokens that can be minted/redeemed within the limit window.
    /// * `limit_window` - Optional time window (in seconds) for the rate limit.
    /// * `bumps` - Bumps for PDA derivation.
    /// # Returns
    /// * `Result<()>` - Result indicating success or failure.
    pub fn initialize_user(
        &mut self,
        rate_limit: Option<u64>,
        limit_window: Option<u64>,
        bumps: &InitializeUserBumps,
    ) -> Result<()> {
        match (rate_limit, limit_window) {
            (Some(rate), Some(window)) => {
                self.ondo_user.set_inner(OndoUser {
                    owner: self.user.key(),
                    mint: self.mint.key(),
                    rate_limit: Some(rate),
                    limit_window: Some(window),
                    mint_capacity_used: Some(0), // Initialize to 0 when rate limits are set
                    mint_last_updated: None,
                    redeem_capacity_used: Some(0), // Initialize to 0 when rate limits are set
                    redeem_last_updated: None,
                    bump: bumps.ondo_user,
                })
            }
            _ => self.ondo_user.set_inner(OndoUser {
                owner: self.user.key(),
                mint: self.mint.key(),
                rate_limit: None,
                limit_window: None,
                mint_capacity_used: None,
                mint_last_updated: None,
                redeem_capacity_used: None,
                redeem_last_updated: None,
                bump: bumps.ondo_user,
            }),
        }

        Ok(())
    }
}
