use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{burn_checked, mint_to, BurnChecked, MintTo},
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    constants::{MAX_MINT_AMOUNT, MINT_AUTHORITY_SEED, USDON_MANAGER_STATE_SEED},
    errors::OndoError,
    events::{RoleGranted, RoleRevoked},
    state::{RoleType, Roles, USDonManagerState},
};

/// Grant a USDon role for a user by creating a `Roles` account
/// Requires `GUARDIAN_USDON` role
/// Only allows `MINTER_ROLE_USDON` and `BURNER_ROLE_USDON` roles to be created
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct USDonGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant the role
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `GUARDIAN_USDON` role
    /// # PDA Seeds
    /// - GUARDIAN_USDON
    /// - The authority's address
    #[account(
        seeds = [RoleType::GUARDIAN_USDON, authority.key().as_ref()],
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

impl<'info> USDonGrantRole<'info> {
    /// Initialize a USDon role for a user
    /// # Arguments
    /// * `role` - The role to grant (must be MinterRoleUsdon, PauserRoleUsdon, or BurnerRoleUsdon)
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully granted, Err otherwise
    pub fn grant_usdon_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &USDonGrantRoleBumps,
    ) -> Result<()> {
        // Only allow `MinterRoleUsdon` and `BurnerRoleUsdon` roles to be created
        require!(
            matches!(role, RoleType::MinterRoleUSDon | RoleType::BurnerRoleUSDon),
            OndoError::InvalidRoleType
        );

        // Write role data to the `Roles` account
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

/// Revoke USDon roles from a user by closing their `Roles` account
/// Requires `GUARDIAN_USDON` role
/// Only allows `MINTER_ROLE_USDON` and `BURNER_ROLE_USDON` roles to be revoked
#[derive(Accounts)]
pub struct USDonRevokeRole<'info> {
    /// The account with the authority to revoke a role
    pub authority: Signer<'info>,

    /// Receives the lamports from closing the `Roles` account
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The Roles account verifying the authority has the `GUARDIAN_USDON` role
    /// # PDA Seeds
    /// - GUARDIAN_USDON
    /// - The authority's address
    #[account(
        seeds = [RoleType::GUARDIAN_USDON, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The Roles account being closed
    /// # PDA Seeds
    /// - Role seed (from RoleType)
    /// - User's address
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

impl<'info> USDonRevokeRole<'info> {
    /// Revoke a USDon role from a user by closing their `Roles` account
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully revoked, Err otherwise
    pub fn revoke_usdon_role(&mut self) -> Result<()> {
        // Validate that the role being revoked is `MinterRoleUsdon` or `BurnerRoleUsdon`
        require!(
            matches!(
                self.role_to_revoke.role,
                RoleType::MinterRoleUSDon | RoleType::BurnerRoleUSDon
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

/// Mint USDon tokens to a specified destination account.
/// Requires `MINTER_ROLE_USDON` or `ADMIN_ROLE_USDON` role.
#[derive(Accounts)]
pub struct USDonMinter<'info> {
    /// The account with the authority to mint USDon tokens
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The mint authority PDA
    /// # PDA Seeds
    /// - MINT_AUTHORITY_SEED
    ///
    /// CHECK: This account is used to verify the mint authority, but does not need to be checked for correctness as it is uninitialized.
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The USDonManagerState account containing USDon configuration
    /// # PDA Seeds
    /// - USDON_MANAGER_STATE_SEED
    #[account(
        seeds = [USDON_MANAGER_STATE_SEED],
        bump = usdon_manager_state.bump,
    )]
    pub usdon_manager_state: Account<'info, USDonManagerState>,

    /// The Roles account verifying the authority has either the `MINTER_ROLE_USDON`
    /// or `ADMIN_ROLE_USDON` role
    /// # PDA Seeds
    /// - Role seed (from the account's role field)
    /// - The authority's address
    #[account(
        seeds = [authority_role_account.role.seed(), authority.key().as_ref()],
        bump = authority_role_account.bump,
        constraint =
            authority_role_account.role == RoleType::MinterRoleUSDon ||
            authority_role_account.role == RoleType::AdminRoleUSDon @
            OndoError::AddressNotFoundInRole
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The USDon mint
    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program,
        address = usdon_manager_state.usdon_mint
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,

    /// The destination token account to mint tokens to
    #[account(mut)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
}

impl<'info> USDonMinter<'info> {
    /// Mint USDon tokens to a destination account (admin function)
    /// Authority must have the `MINTER_ROLE_USDON` or `ADMIN_ROLE_USDON` role
    /// # Arguments
    /// * `amount` - The amount of USDon tokens to mint (must be greater than 0)
    /// * `bump` - The PDA bump for the mint authority
    /// # Returns
    /// * `Result<()>` - Ok if tokens are successfully minted, Err otherwise
    pub fn mint_usdon(&mut self, amount: u64, bump: u8) -> Result<()> {
        // Validate amount
        require_gt!(amount, 0, OndoError::InvalidAmount);

        // Validate amount does not exceed maximum mint amount (USD 10mn notional)
        require_gte!(
            MAX_MINT_AMOUNT,
            amount,
            OndoError::AmountExceedsMaxMintAmount
        );

        // Mint USDon to the destination account
        // Uses the mint authority PDA to sign
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

/// Burn USDon tokens from a specified token account.
/// Requires `BURNER_ROLE_USDON` or `ADMIN_ROLE_USDON` role.
#[derive(Accounts)]
pub struct USDonBurner<'info> {
    /// The account with the authority to burn USDon tokens
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The permanent delegate PDA (also the mint authority)
    /// # PDA Seeds
    /// - MINT_AUTHORITY_SEED
    ///
    /// CHECK: This account is used to verify the mint authority, but does not need to be checked for correctness as it is uninitialized.
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump
    )]
    pub permanent_delegate: UncheckedAccount<'info>,

    /// The USDonManagerState account containing USDon configuration
    /// # PDA Seeds
    /// - USDON_MANAGER_STATE_SEED
    #[account(
        seeds = [USDON_MANAGER_STATE_SEED],
        bump = usdon_manager_state.bump,
    )]
    pub usdon_manager_state: Account<'info, USDonManagerState>,

    /// The `Roles` account verifying the authority has either the `BURNER_ROLE_USDON` role
    /// or the `ADMIN_ROLE_USDON` role
    /// # PDA Seeds
    /// - Role seed (from the account's role field)
    /// - The authority's address
    #[account(
        seeds = [authority_role_account.role.seed(), authority.key().as_ref()],
        bump = authority_role_account.bump,
        constraint =
            authority_role_account.role == RoleType::BurnerRoleUSDon ||
            authority_role_account.role == RoleType::AdminRoleUSDon @
            OndoError::AddressNotFoundInRole
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The USDon mint
    #[account(
        mut,
        mint::authority = permanent_delegate,
        mint::token_program = token_program,
        address = usdon_manager_state.usdon_mint
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,

    /// The token account to burn tokens from
    #[account(
        mut,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub destination: InterfaceAccount<'info, TokenAccount>,
}

impl<'info> USDonBurner<'info> {
    /// Burn USDon tokens from a token account (admin function)
    /// Authority must have either the `BURNER_ROLE_USDON` or `ADMIN_ROLE_USDON` role
    /// # Arguments
    /// * `amount` - The amount of USDon tokens to burn (must be greater than 0)
    /// * `bump` - The PDA bump for the permanent delegate
    /// # Returns
    /// * `Result<()>` - Ok if tokens are successfully burned, Err otherwise
    pub fn burn_usdon(&mut self, amount: u64, bump: u8) -> Result<()> {
        // Validate amount
        require_gt!(amount, 0, OndoError::InvalidAmount);

        // Burn USDon from the destination account
        burn_checked(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                BurnChecked {
                    mint: self.mint.to_account_info(),
                    from: self.destination.to_account_info(),
                    authority: self.permanent_delegate.to_account_info(),
                },
                &[&[MINT_AUTHORITY_SEED, &[bump]]],
            ),
            amount,
            self.mint.decimals,
        )
    }
}
