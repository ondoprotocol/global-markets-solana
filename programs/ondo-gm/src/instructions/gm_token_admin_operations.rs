use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::{mint_to, MintTo},
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_token_2022::extension::pausable::instruction::{pause, resume};

use crate::{
    constants::{
        MAX_MINT_AMOUNT, MINT_AUTHORITY_SEED, ORACLE_SANITY_CHECK_SEED, PRICE_SCALING_FACTOR,
        USDON_MANAGER_STATE_SEED,
    },
    errors::OndoError,
    events::{GMTokenPaused, RoleGranted, RoleRevoked},
    state::{OracleSanityCheck, RoleType, Roles, USDonManagerState},
    utils::mul_div,
};

/// Grant a GM Token role to a user by initializing a `Roles` account
/// Requires `ADMIN_ROLE_GMTOKEN` role
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct GMTokenGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant GM Token roles
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN` role
    /// # PDA Seeds
    /// - ADMIN_ROLE_GMTOKEN
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The new Roles account being created for the user
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

impl<'info> GMTokenGrantRole<'info> {
    /// Grant a GM Token role to a user
    /// # Arguments
    /// * `role` - The role to grant (must be `MinterRoleGmtoken`, `PauserRoleGmtoken`, or `UnpauserRoleGmtoken`)
    /// * `user` - The public key of the user to grant the role to
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully granted, Err otherwise
    pub fn grant_gmtoken_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &GMTokenGrantRoleBumps,
    ) -> Result<()> {
        // Validate that the role being added is `MinterRoleGmtoken`, `PauserRoleGmtoken`, or `UnpauserRoleGmtoken`
        require!(
            matches!(
                role,
                RoleType::MinterRoleGMToken
                    | RoleType::PauserRoleGMToken
                    | RoleType::UnpauserRoleGMToken
            ),
            OndoError::InvalidRoleType
        );

        // Write to the Roles account
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

/// Revoke a GM Token role from a user by closing their `Roles` account
/// Requires `ADMIN_ROLE_GMTOKEN` role
#[derive(Accounts)]
pub struct GMTokenRevokeRole<'info> {
    /// The recipient of the closed account lamports
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The account with the authority to revoke GM Token roles
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN` role
    /// # PDA Seeds
    /// - ADMIN_ROLE_GMTOKEN
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN, authority.key().as_ref()],
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
}

impl<'info> GMTokenRevokeRole<'info> {
    /// Revoke a GM Token role from a user by closing their `Roles` account
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully revoked, Err otherwise
    pub fn revoke_gmtoken_role(&mut self) -> Result<()> {
        // Validate that the role being removed is `MinterRoleGmtoken`, `PauserRoleGmtoken`, or `UnpauserRoleGmtoken`
        require!(
            matches!(
                self.role_to_revoke.role,
                RoleType::MinterRoleGMToken
                    | RoleType::PauserRoleGMToken
                    | RoleType::UnpauserRoleGMToken
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

    /// The `OracleSanityCheck` account validating oracle price updates
    /// # PDA Seeds
    /// - `ORACLE_SANITY_CHECK_SEED`
    /// - Mint address
    #[account(
        mut,
        seeds = [ORACLE_SANITY_CHECK_SEED, mint.key().as_ref()],
        bump = oracle_sanity_check.bump,
        has_one = mint @ OndoError::InvalidInputMint
    )]
    pub oracle_sanity_check: Account<'info, OracleSanityCheck>,

    /// The mint authority PDA
    /// # PDA Seeds
    /// - `MINT_AUTHORITY_SEED`
    ///
    /// CHECK: This account is used to verify the mint authority, but does not need to be checked for correctness as it is uninitialized.
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

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,
    /// The associated token program
    pub associated_token_program: Program<'info, AssociatedToken>,
    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> GMTokenMinter<'info> {
    /// Mint GM tokens to a user's account
    /// # Arguments
    /// * `amount` - The amount of tokens to mint (must be greater than 0)
    /// * `bump` - The PDA bump for the mint authority
    /// # Returns
    /// * `Result<()>` - Ok if tokens are successfully minted, Err otherwise
    pub fn mint_gm(&mut self, amount: u64, bump: u8) -> Result<()> {
        // Validate amount is greater than 0
        require_gt!(amount, 0, OndoError::InvalidAmount);

        // Calculate notional USD value: (amount × price) / PRICE_SCALING_FACTOR
        let notional_usd = mul_div(
            amount,
            self.oracle_sanity_check.last_price,
            PRICE_SCALING_FACTOR as u64,
            true,
        )?;

        // Validate notional USD value does not exceed $10 million
        require_gte!(
            MAX_MINT_AMOUNT,
            notional_usd,
            OndoError::AmountExceedsMaxMintAmount
        );

        // Mint GM Tokens to the destination account
        // using the mint authority PDA as signer
        mint_to(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                MintTo {
                    mint: self.mint.to_account_info(),
                    to: self.destination.to_account_info(),
                    authority: self.mint_authority.to_account_info(),
                },
                &[&[MINT_AUTHORITY_SEED, &[bump]]],
            ),
            amount,
        )
    }
}

/// Pause a GM token mint (disables all minting, burning, and transferring)
/// Requires `PAUSER_ROLE_GMTOKEN` role
#[derive(Accounts)]
pub struct PauseGMToken<'info> {
    /// The account with the authority to execute the pause operation
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `PAUSER_ROLE_GMTOKEN` role
    /// # PDA Seeds
    /// - `PAUSER_ROLE_GMTOKEN`
    /// - The authority's address
    #[account(
        seeds = [RoleType::PAUSER_ROLE_GMTOKEN, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The GM Token mint to pause
    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// The mint authority PDA that has pausable authority
    /// # PDA Seeds
    /// - MINT_AUTHORITY_SEED
    ///
    /// CHECK: Validated by spl_token_2022::extension::pausable::instruction::pause execution
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> PauseGMToken<'info> {
    /// Pause a GM token mint (disables all minting, burning, and transferring)
    /// # Arguments
    /// * `bump` - The PDA bump for the mint authority
    /// # Returns
    /// * `Result<()>` - Ok if the mint is successfully paused, Err otherwise
    pub fn pause(&self, bump: u8) -> Result<()> {
        // Create the pause instruction
        let pause_ix = pause(
            &self.token_program.key(),
            &self.mint.key(),
            &self.mint_authority.key(),
            &[],
        )?;

        // Execute with PDA signer
        invoke_signed(
            &pause_ix,
            &[
                self.token_program.to_account_info(),
                self.mint.to_account_info(),
                self.mint_authority.to_account_info(),
            ],
            &[&[MINT_AUTHORITY_SEED, &[bump]]],
        )?;

        emit!(GMTokenPaused {
            is_paused: true,
            token: self.mint.key(),
            pauser: self.authority.key()
        });

        Ok(())
    }
}

/// Resume a GM token mint (enables all minting, burning, and transferring)
/// Requires `UNPAUSER_ROLE_GMTOKEN` role
#[derive(Accounts)]
pub struct ResumeGMToken<'info> {
    /// The account with the authority to execute the resume operation
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `UNPAUSER_ROLE_GMTOKEN` role
    /// # PDA Seeds
    /// - `UNPAUSER_ROLE_GMTOKEN`
    /// - The authority's address
    #[account(
        seeds = [RoleType::UNPAUSER_ROLE_GMTOKEN, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The GM Token mint to resume
    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// The mint authority PDA that has pausable authority
    /// # PDA Seeds
    /// - `MINT_AUTHORITY_SEED`
    ///
    /// CHECK: Validated by spl_token_2022::extension::pausable::instruction::resume execution
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> ResumeGMToken<'info> {
    /// Resume a GM token mint (enables all minting, burning, and transferring)
    /// # Arguments
    /// * `bump` - The PDA bump for the mint authority
    /// # Returns
    /// * `Result<()>` - Ok if the mint is successfully resumed, Err otherwise
    pub fn resume(&self, bump: u8) -> Result<()> {
        // Create the resume instruction
        let resume_ix = resume(
            &self.token_program.key(),
            &self.mint.key(),
            &self.mint_authority.key(),
            &[],
        )?;

        // Execute with PDA signer
        invoke_signed(
            &resume_ix,
            &[
                self.token_program.to_account_info(),
                self.mint.to_account_info(),
                self.mint_authority.to_account_info(),
            ],
            &[&[MINT_AUTHORITY_SEED, &[bump]]],
        )?;

        emit!(GMTokenPaused {
            is_paused: false,
            token: self.mint.key(),
            pauser: self.authority.key()
        });

        Ok(())
    }
}
