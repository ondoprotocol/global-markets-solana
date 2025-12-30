use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::ID as INSTRUCTIONS_ID;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use super::TokenManager;
use crate::{
    constants::{
        ATTESTATION_ID_SEED, GMTOKEN_MANAGER_STATE_SEED, MINT_AUTHORITY_SEED, ONDO_USER_SEED,
        ORACLE_SANITY_CHECK_SEED, TOKEN_LIMIT_ACCOUNT_SEED, USDON_MANAGER_STATE_SEED,
        WHITELIST_SEED,
    },
    state::{GMTokenManagerState, OndoUser, OracleSanityCheck, TokenLimit, USDonManagerState},
};

#[event_cpi]
#[derive(Accounts)]
#[instruction(attestation_id: [u8; 16])]
pub struct USDonSwapContext<'info> {
    /// The user performing the USDon swap, pays for account creation if needed
    #[account(mut)]
    pub user: Signer<'info>,

    /// The GM Token mint involved in the swap
    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// The mint authority PDA
    /// # PDA Seeds
    /// - MINT_AUTHORITY_SEED
    /// CHECK: This account is used to verify the mint authority.
    /// Does not need to be checked for correctness as it is uninitialized.
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The OndoUser account tracking user-specific state for this mint
    /// # PDA Seeds
    /// - ONDO_USER_SEED
    /// - User's address
    /// - Mint address
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + OndoUser::INIT_SPACE,
        seeds = [ONDO_USER_SEED, user.key().as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub ondo_user: Box<Account<'info, OndoUser>>,

    /// The TokenLimit account enforcing mint/burn limits for the GM Token
    /// # PDA Seeds
    /// - TOKEN_LIMIT_ACCOUNT_SEED
    /// - Mint address
    #[account(
        mut,
        seeds = [TOKEN_LIMIT_ACCOUNT_SEED, mint.key().as_ref()],
        bump = token_limit_account.bump,
    )]
    pub token_limit_account: Box<Account<'info, TokenLimit>>,

    /// The OracleSanityCheck account validating oracle price updates
    /// # PDA Seeds
    /// - ORACLE_SANITY_CHECK_SEED
    /// - Mint address
    #[account(
        mut,
        seeds = [ORACLE_SANITY_CHECK_SEED, mint.key().as_ref()],
        bump = sanity_check_account.bump,
    )]
    pub sanity_check_account: Box<Account<'info, OracleSanityCheck>>,

    /// The user's associated token account for the GM Token
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The attestation ID account preventing attestation reuse
    /// # PDA Seeds
    /// - ATTESTATION_ID_SEED
    /// - Attestation ID (16-byte array)
    /// CHECK: Seeds constraint validates PDA address.
    /// Existence means the attestation has been used.
    #[account(
        mut,
        seeds = [ATTESTATION_ID_SEED, attestation_id.as_ref()],
        bump,
    )]
    pub attestation_id_account: UncheckedAccount<'info>,

    /// The Whitelist account verifying the user is authorized
    /// # PDA Seeds
    /// - WHITELIST_SEED
    /// - User's address
    /// CHECK: Seeds constraint validates PDA address.
    /// Validated in instruction handler - returns UserNotWhitelisted if not initialized.
    #[account(
        seeds = [WHITELIST_SEED, user.key().as_ref()],
        bump,
    )]
    pub whitelist: UncheckedAccount<'info>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,

    /// The system program
    pub system_program: Program<'info, System>,

    /// The associated token program
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The USDon vault storing USDon tokens received from users during swaps
    #[account(
        mut,
        associated_token::mint = usdon_mint,
        associated_token::authority = usdon_manager_state,
        associated_token::token_program = token_program,
        constraint = usdon_vault.key() == usdon_manager_state.usdon_vault
    )]
    pub usdon_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The USDon mint (Token-2022)
    #[account(
        mut,
        mint::token_program = token_program,
        constraint = usdon_mint.key() == usdon_manager_state.usdon_mint
    )]
    pub usdon_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The user's USDon token account
    #[account(mut)]
    pub user_usdon_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The USDonManagerState account used as authority for vault operations
    /// # PDA Seeds
    /// - USDON_MANAGER_STATE_SEED
    #[account(
        seeds = [USDON_MANAGER_STATE_SEED],
        bump = usdon_manager_state.bump,
    )]
    pub usdon_manager_state: Box<Account<'info, USDonManagerState>>,

    /// The GmTokenManagerState account managing GM Token operations
    /// - Stores protocol parameters like factory, redemption, and minting paused.
    /// # PDA Seeds
    /// - GMTOKEN_MANAGER_STATE_SEED
    #[account(
        mut,
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
    )]
    pub gmtoken_manager_state: Box<Account<'info, GMTokenManagerState>>,

    /// CHECK: Sysvar account for instruction introspection
    #[account(address = INSTRUCTIONS_ID)]
    instructions: UncheckedAccount<'info>,
}

impl<'info> USDonSwapContext<'info> {
    /// Creates a TokenManager instance from the current context.
    /// This TokenManager facilitates token operations within the USDon swap context.
    /// # Returns
    /// * `TokenManager` - A TokenManager instance with references to the relevant accounts.
    /// # Safety
    /// This method uses &mut self to provide mutable references to the accounts,
    /// ensuring that the TokenManager can perform necessary operations safely.
    #[allow(clippy::wrong_self_convention)]
    pub fn into_token_manager(&mut self) -> TokenManager<'_, 'info> {
        TokenManager {
            user: &mut self.user,
            mint: &mut self.mint,
            mint_authority: &self.mint_authority,
            ondo_user: &mut self.ondo_user,
            token_limit_account: &mut self.token_limit_account,
            sanity_check_account: &mut self.sanity_check_account,
            user_token_account: &mut self.user_token_account,
            attestation_id_account: &mut self.attestation_id_account,
            whitelist: &self.whitelist,
            token_program: &self.token_program,
            system_program: &self.system_program,
            associated_token_program: &self.associated_token_program,
            spl_token_program: None,
            usdc_price_update: None,
            usdc_vault: None,
            usdon_vault: &mut self.usdon_vault,
            usdc_mint: None,
            user_usdc_token_account: None,
            usdon_mint: &self.usdon_mint,
            user_usdon_token_account: &mut self.user_usdon_token_account,
            usdon_manager_state: &self.usdon_manager_state,
            gmtoken_manager_state: &mut self.gmtoken_manager_state,
            instructions: &self.instructions,
        }
    }
}
