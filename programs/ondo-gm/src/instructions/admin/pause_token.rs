use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::token_interface::{Mint, TokenInterface};
use spl_token_2022::extension::pausable::instruction::{pause, resume};

use crate::{
    constants::{MINT_AUTHORITY_SEED, PORTFOLIO_MINT_AUTHORITY_SEED},
    events::TokenPaused,
    instructions::TokenKind,
    state::{RoleType, Roles},
};

/// Pause a GM token mint (disables all minting, burning, and transferring)
/// Requires `PAUSER_ROLE_GMTOKEN` role
#[derive(Accounts)]
pub struct PauseGMToken<'info> {
    /// The account with the authority to execute the pause operation
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
    const SEED: &'static [u8] = TokenKind::GM.seed();

    /// Pause a GM token mint (disables all minting, burning, and transferring)
    /// # Arguments
    /// * `bump` - The PDA bump for the mint authority
    /// # Returns
    /// * `Result<()>` - Ok if the mint is successfully paused, Err otherwise
    pub fn pause(&self, bump: u8) -> Result<()> {
        set_paused(
            PauseParams {
                authority: &self.authority,
                mint: &self.mint,
                mint_authority: &self.mint_authority,
                token_program: &self.token_program,
                seed: Self::SEED,
                bump,
            },
            true,
            TokenKind::GM,
        )
    }
}

/// Resume a GM token mint (enables all minting, burning, and transferring)
/// Requires `UNPAUSER_ROLE_GMTOKEN` role
#[derive(Accounts)]
pub struct ResumeGMToken<'info> {
    /// The account with the authority to execute the resume operation
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
    const SEED: &'static [u8] = TokenKind::GM.seed();

    /// Resume a GM token mint (enables all minting, burning, and transferring)
    /// # Arguments
    /// * `bump` - The PDA bump for the mint authority
    /// # Returns
    /// * `Result<()>` - Ok if the mint is successfully resumed, Err otherwise
    pub fn resume(&self, bump: u8) -> Result<()> {
        set_paused(
            PauseParams {
                authority: &self.authority,
                mint: &self.mint,
                mint_authority: &self.mint_authority,
                token_program: &self.token_program,
                seed: Self::SEED,
                bump,
            },
            false,
            TokenKind::GM,
        )
    }
}

/// Pause a Portfolio token mint (disables all minting, burning, and transferring)
/// Requires `PAUSER_ROLE_PORTFOLIO_TOKEN` role
#[derive(Accounts)]
pub struct PausePortfolioToken<'info> {
    /// The account with the authority to execute the pause operation
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `PAUSER_ROLE_PORTFOLIO_TOKEN` role
    #[account(
        seeds = [RoleType::PAUSER_ROLE_PORTFOLIO_TOKEN, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The Portfolio Token mint to pause
    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// The mint authority PDA that has pausable authority
    /// CHECK: Validated by spl_token_2022::extension::pausable::instruction::pause execution
    #[account(
        seeds = [PORTFOLIO_MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> PausePortfolioToken<'info> {
    const SEED: &'static [u8] = TokenKind::Portfolio.seed();

    /// Pause a portfolio token mint (disables all minting, burning, and transferring)
    /// # Arguments
    /// * `bump` - The PDA bump for the portfolio mint authority
    /// # Returns
    /// * `Result<()>` - Ok if the mint is successfully paused, Err otherwise
    pub fn pause(&self, bump: u8) -> Result<()> {
        set_paused(
            PauseParams {
                authority: &self.authority,
                mint: &self.mint,
                mint_authority: &self.mint_authority,
                token_program: &self.token_program,
                seed: Self::SEED,
                bump,
            },
            true,
            TokenKind::Portfolio,
        )
    }
}

/// Resume a Portfolio Token mint (enables all minting, burning, and transferring)
/// Requires `UNPAUSER_ROLE_PORTFOLIO_TOKEN` role
#[derive(Accounts)]
pub struct ResumePortfolioToken<'info> {
    /// The account with the authority to execute the resume operation
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `UNPAUSER_ROLE_PORTFOLIO_TOKEN` role
    #[account(
        seeds = [RoleType::UNPAUSER_ROLE_PORTFOLIO_TOKEN, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The Portfolio Token mint to resume
    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// The mint authority PDA that has pausable authority
    /// CHECK: Validated by spl_token_2022::extension::pausable::instruction::resume execution
    #[account(
        seeds = [PORTFOLIO_MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> ResumePortfolioToken<'info> {
    const SEED: &'static [u8] = TokenKind::Portfolio.seed();

    /// Resume a portfolio token mint (enables all minting, burning, and transferring)
    /// # Arguments
    /// * `bump` - The PDA bump for the portfolio mint authority
    /// # Returns
    /// * `Result<()>` - Ok if the mint is successfully resumed, Err otherwise
    pub fn resume(&self, bump: u8) -> Result<()> {
        set_paused(
            PauseParams {
                authority: &self.authority,
                mint: &self.mint,
                mint_authority: &self.mint_authority,
                token_program: &self.token_program,
                seed: Self::SEED,
                bump,
            },
            false,
            TokenKind::Portfolio,
        )
    }
}

pub struct PauseParams<'a, 'info> {
    authority: &'a Signer<'info>,
    mint: &'a InterfaceAccount<'info, Mint>,
    mint_authority: &'a AccountInfo<'info>,
    token_program: &'a Interface<'info, TokenInterface>,
    seed: &'static [u8],
    bump: u8,
}

fn set_paused(params: PauseParams, is_paused: bool, kind: TokenKind) -> Result<()> {
    let PauseParams {
        authority,
        mint,
        mint_authority,
        token_program,
        seed,
        bump,
    } = params;

    let ix = if is_paused {
        pause(
            &token_program.key(),
            &mint.key(),
            &mint_authority.key(),
            &[],
        )?
    } else {
        resume(
            &token_program.key(),
            &mint.key(),
            &mint_authority.key(),
            &[],
        )?
    };

    invoke_signed(
        &ix,
        &[
            token_program.to_account_info(),
            mint.to_account_info(),
            mint_authority.to_account_info(),
        ],
        &[&[seed, &[bump]]],
    )?;

    emit!(TokenPaused {
        is_paused,
        token: mint.key(),
        pauser: authority.key(),
        kind: kind as u8
    });

    Ok(())
}
