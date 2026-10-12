#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use anchor_lang::prelude::*;
mod constants;
mod errors;
mod events;
mod instructions;
pub mod security;
mod state;
mod utils;

use events::TradeExecuted;
use instructions::*;
use state::RoleType;

#[cfg(feature = "devnet")]
declare_id!("sSV6QQi2UTvjmPx4UMLDFJas9CQE3VmBz64wPJHN1gm");
#[cfg(feature = "testnet")]
declare_id!("xoVUinQWoi4Bxre6oqEJHp9WrJaHacs74gtYtondogm");
#[cfg(feature = "mainnet")]
declare_id!("XzTT4XB8m7sLD2xi6snefSasaswsKCxx5Tifjondogm");
#[cfg(not(any(feature = "mainnet", feature = "devnet", feature = "testnet")))]
declare_id!("9ZtajufGgF66yPKmQSq4gCUavfCJjGUBeeQV5hAkNtS1");

#[program]
pub mod ondo_gm {
    use super::*;

    /// Initialize the USDon manager state
    ///
    /// Sets up the manager with the USDon mint, USDC oracle guard configuration,
    /// and vault addresses for USDC and USDon tokens.
    /// Signer must have the GUARDIAN_USDON role
    pub fn initialize_usdon_manager(
        ctx: Context<InitializeUSDonManager>,
        oracle_price_enabled: bool,
        oracle_price_max_age: u64,
        usdc_price_update_address: Pubkey,
    ) -> Result<()> {
        ctx.accounts.initialize_usdon_manager(
            oracle_price_enabled,
            oracle_price_max_age,
            usdc_price_update_address,
            &ctx.bumps,
        )
    }

    /// Initialize the GM token manager state
    ///
    /// Sets up the manager with pause states for factory, redemptions, and minting,
    /// and configures the secp256k1 attestation signer address. The trading-hours
    /// offset is retained for account compatibility; trading hours are not enforced.
    /// Signer must have the ADMIN_ROLE_GMTOKEN_MANAGER role
    pub fn initialize_gmtoken_manager(
        ctx: Context<InitializeGMTokenManager>,
        factory_paused: bool,
        redemptions_paused: bool,
        minting_paused: bool,
        attestation_signer_secp: [u8; 20],
        trading_hours_offset: i64,
    ) -> Result<()> {
        ctx.accounts.initialize_gmtoken_manager(
            factory_paused,
            redemptions_paused,
            minting_paused,
            attestation_signer_secp,
            trading_hours_offset,
            &ctx.bumps,
        )
    }

    /// Initialize the Portfolio token manager state
    ///
    /// The Portfolio equivalent of `initialize_gmtoken_manager`. Portfolio tokens
    /// have their own manager state, so their factory and global pause flags are
    /// independent of the GM Token ones.
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER role
    pub fn initialize_portfolio_token_manager(
        ctx: Context<InitializePortfolioTokenManager>,
        factory_paused: bool,
        redemption_paused: bool,
        minting_paused: bool,
        attestation_signer_secp: [u8; 20],
    ) -> Result<()> {
        ctx.accounts.initialize_portfolio_token_manager(
            factory_paused,
            redemption_paused,
            minting_paused,
            attestation_signer_secp,
            &ctx.bumps,
        )
    }

    /// Enable or disable the USDC oracle guard for GM and portfolio USDC trades
    /// Signer must have the ADMIN_ROLE_USDON_MANAGER role
    pub fn enable_oracle_price(ctx: Context<USDonManagerAdmin>, is_enabled: bool) -> Result<()> {
        ctx.accounts.enable_oracle_price(is_enabled)
    }

    /// Set the maximum age for oracle price data. When USDC oracle prices are more stale than `oracle_price_max_age`
    /// USDC trades will halt if the oracle guard is enabled.
    /// Signer must have the ADMIN_ROLE_USDON_MANAGER role
    pub fn set_oracle_price_max_age(
        ctx: Context<USDonManagerAdmin>,
        oracle_price_max_age: u64,
    ) -> Result<()> {
        ctx.accounts.set_oracle_price_max_age(oracle_price_max_age)
    }

    /// Set the USDC price update address
    /// Signer must have the ADMIN_ROLE_USDON_MANAGER role
    pub fn set_usdc_price_update_address(
        ctx: Context<USDonManagerAdmin>,
        new_price_update_address: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .set_usdc_price_update_address(new_price_update_address)
    }

    /// Retrieve (withdraw) tokens from a vault controlled by the USDon manager
    ///
    /// Allows admins to withdraw any tokens (USDC, USDon, etc.) from vaults
    /// owned by the usdon_manager_state PDA.
    /// Signer must have the ADMIN_ROLE_USDON_MANAGER role
    pub fn retrieve_tokens(ctx: Context<RetrieveTokens>, amount: u64) -> Result<()> {
        ctx.accounts.retrieve_tokens(amount)
    }

    /// Initialize a user account with the mint's default user limits
    /// Signer must have the INITIALIZER_ROLE_ONDO_USER role
    pub fn initialize_user(ctx: Context<InitializeUser>) -> Result<()> {
        ctx.accounts.initialize_user(&ctx.bumps)
    }

    /// Initialize token-level rate limits
    ///
    /// Sets rate limits for the token and default limits for users trading this token.
    /// Signer must have the DEPLOYER_ROLE_TOKEN_FACTORY role
    pub fn initialize_token_limit(
        ctx: Context<InitializeTokenLimit>,
        rate_limit: Option<u64>,
        limit_window: Option<u64>,
        default_user_rate_limit: Option<u64>,
        default_limit_window: Option<u64>,
    ) -> Result<()> {
        ctx.accounts.initialize_token_limit(
            rate_limit,
            limit_window,
            default_user_rate_limit,
            default_limit_window,
            &ctx.bumps,
        )
    }

    /// Update token-level rate limits
    /// Signer must have the ADMIN_ROLE_TOKEN_LIMIT role
    pub fn set_token_limit(
        ctx: Context<SetTokenLimit>,
        rate_limit: Option<u64>,
        limit_window: Option<u64>,
        default_user_rate_limit: Option<u64>,
        default_user_limit_window: Option<u64>,
    ) -> Result<()> {
        ctx.accounts.set_token_limit(
            rate_limit,
            limit_window,
            default_user_rate_limit,
            default_user_limit_window,
        )
    }

    /// Initialize sanity check parameters for a token
    ///
    /// Sets up price deviation and time delay checks to ensure safe trading.
    /// Signer must have the DEPLOYER_ROLE_TOKEN_FACTORY role
    pub fn initialize_sanity_check(
        ctx: Context<InitializeSanityCheck>,
        last_price: u64,
        allowed_deviation_bps: u64,
        max_time_delay: i64,
    ) -> Result<()> {
        ctx.accounts.initialize_sanity_check(
            last_price,
            allowed_deviation_bps,
            max_time_delay,
            &ctx.bumps,
        )
    }

    /// Mint GM tokens by paying with USDon
    ///
    /// Requires a valid attestation with price, amount, and expiration.
    pub fn mint_with_usdon(
        ctx: Context<USDonSwapContext>,
        attestation_id: [u8; 16],
        price: u64,
        amount: u64,
        expiration: i64,
    ) -> Result<()> {
        token_manager::mint_with_attestation(
            &mut ctx.accounts.as_token_manager(),
            attestation_id,
            price,
            amount,
            expiration,
            true,
            ctx.bumps.ondo_user,
            ctx.bumps.attestation_id_account,
            ctx.bumps.mint_authority,
        )?;

        emit_cpi!(TradeExecuted {
            execution_id: ctx.accounts.gmtoken_manager_state.next_execution_id()?,
        });

        Ok(())
    }

    /// Mint GM tokens by paying with USDC
    ///
    /// Requires a valid attestation with price, amount, and expiration.
    pub fn mint_with_usdc(
        ctx: Context<USDCSwapContext>,
        attestation_id: [u8; 16],
        price: u64,
        amount: u64,
        expiration: i64,
    ) -> Result<()> {
        token_manager::mint_with_attestation(
            &mut ctx.accounts.as_token_manager(),
            attestation_id,
            price,
            amount,
            expiration,
            false,
            ctx.bumps.ondo_user,
            ctx.bumps.attestation_id_account,
            ctx.bumps.mint_authority,
        )?;

        emit_cpi!(TradeExecuted {
            execution_id: ctx.accounts.gmtoken_manager_state.next_execution_id()?,
        });

        Ok(())
    }

    /// Redeem GM tokens for USDon
    ///
    /// Requires a valid attestation with price, amount, and expiration.
    pub fn redeem_for_usdon(
        ctx: Context<USDonSwapContext>,
        attestation_id: [u8; 16],
        price: u64,
        amount: u64,
        expiration: i64,
    ) -> Result<()> {
        token_manager::redeem_with_attestation(
            &mut ctx.accounts.as_token_manager(),
            attestation_id,
            price,
            amount,
            expiration,
            true,
            ctx.bumps.ondo_user,
            ctx.bumps.attestation_id_account,
            ctx.bumps.mint_authority,
        )?;

        emit_cpi!(TradeExecuted {
            execution_id: ctx.accounts.gmtoken_manager_state.next_execution_id()?,
        });

        Ok(())
    }

    /// Redeem GM tokens for USDC
    ///
    /// Requires a valid attestation with price, amount, and expiration.
    pub fn redeem_for_usdc(
        ctx: Context<USDCSwapContext>,
        attestation_id: [u8; 16],
        price: u64,
        amount: u64,
        expiration: i64,
    ) -> Result<()> {
        token_manager::redeem_with_attestation(
            &mut ctx.accounts.as_token_manager(),
            attestation_id,
            price,
            amount,
            expiration,
            false,
            ctx.bumps.ondo_user,
            ctx.bumps.attestation_id_account,
            ctx.bumps.mint_authority,
        )?;

        emit_cpi!(TradeExecuted {
            execution_id: ctx.accounts.gmtoken_manager_state.next_execution_id()?,
        });

        Ok(())
    }

    /// Mint Portfolio tokens by paying with USDC
    ///
    /// Requires a valid attestation with price, amount, and expiration
    pub fn mint_pt_with_attestation(
        ctx: Context<PortfolioTokenManager>,
        attestation_id: [u8; 16],
        price: u64,
        amount: u64,
        expiration: i64,
    ) -> Result<()> {
        portfolio_token_manager::mint_with_attestation(
            ctx,
            attestation_id,
            price,
            amount,
            expiration,
        )
    }

    /// Redeem Portfolio tokens for USDC
    ///
    /// Requires a valid attestation with price, amount, and expiration
    pub fn redeem_pt_with_attestation(
        ctx: Context<PortfolioTokenManager>,
        attestation_id: [u8; 16],
        price: u64,
        amount: u64,
        expiration: i64,
    ) -> Result<()> {
        portfolio_token_manager::redeem_with_attestation(
            ctx,
            attestation_id,
            price,
            amount,
            expiration,
        )
    }

    /// Add an address to the whitelist
    /// Signer must have the ADMIN_ROLE_WHITELIST role
    pub fn add_to_whitelist(
        ctx: Context<AddToWhitelist>,
        address_to_whitelist: Pubkey,
    ) -> Result<()> {
        ctx.accounts.add_to_whitelist(address_to_whitelist)
    }

    /// Remove an address from the whitelist
    /// Signer must have the ADMIN_ROLE_WHITELIST role
    pub fn remove_from_whitelist(
        ctx: Context<RemoveFromWhitelist>,
        address_to_remove: Pubkey,
    ) -> Result<()> {
        ctx.accounts.remove_from_whitelist(address_to_remove)
    }

    /// Grants the specified role to a user
    /// The signer must be the upgrade authority of the program
    pub fn grant_role(ctx: Context<GrantRole>, role: RoleType, user: Pubkey) -> Result<()> {
        ctx.accounts.grant_role(role, user, &ctx.bumps)
    }

    /// Grants the specified USDon role to a user
    /// Signer must have the GUARDIAN_USDON role
    pub fn grant_usdon_role(
        ctx: Context<USDonGrantRole>,
        role: RoleType,
        user: Pubkey,
    ) -> Result<()> {
        ctx.accounts.grant_usdon_role(role, user, &ctx.bumps)
    }

    /// Revokes the specified USDon role from a user
    /// Signer must have the GUARDIAN_USDON role
    pub fn revoke_usdon_role(ctx: Context<USDonRevokeRole>) -> Result<()> {
        ctx.accounts.revoke_usdon_role()
    }

    /// Mint USDon tokens (admin function)
    /// Signer must have the MINTER_ROLE_USDON or ADMIN_ROLE_USDON role
    pub fn mint_usdon(ctx: Context<USDonMinter>, amount: u64) -> Result<()> {
        ctx.accounts.mint_usdon(amount, ctx.bumps.mint_authority)
    }

    /// Burn USDon tokens (admin function)
    /// Signer must have the BURNER_ROLE_USDON or ADMIN_ROLE_USDON role
    pub fn burn_usdon(ctx: Context<USDonBurner>, amount: u64) -> Result<()> {
        ctx.accounts
            .burn_usdon(amount, ctx.bumps.permanent_delegate)
    }

    /// Mint GM tokens directly (admin function)
    /// Requires unpaused minting and a fresh sanity-check price; capped at $10 million notional.
    /// Signer must have the MINTER_ROLE_GMTOKEN role
    pub fn mint_gm(ctx: Context<GMTokenMinter>, amount: u64) -> Result<()> {
        ctx.accounts.mint_gm(amount, ctx.bumps.mint_authority)
    }

    /// Mint Portfolio tokens directly (admin function)
    /// Requires unpaused minting and a fresh sanity-check price; capped at $10 million notional.
    /// Signer must have the MINTER_ROLE_PORTFOLIO_TOKEN role
    pub fn mint_portfolio(ctx: Context<PortfolioTokenMinter>, amount: u64) -> Result<()> {
        ctx.accounts
            .mint_portfolio(amount, ctx.bumps.mint_authority)
    }

    /// Grants the specified GMToken role to the user
    /// Signer must have the ADMIN_ROLE_GMTOKEN role
    pub fn grant_gmtoken_role(
        ctx: Context<GMTokenGrantRole>,
        role: RoleType,
        user: Pubkey,
    ) -> Result<()> {
        ctx.accounts.grant_gmtoken_role(role, user, &ctx.bumps)
    }

    /// Revokes the specified GMToken role from the user
    /// Signer must have the ADMIN_ROLE_GMTOKEN role
    pub fn revoke_gmtoken_role(ctx: Context<GMTokenRevokeRole>) -> Result<()> {
        ctx.accounts.revoke_gmtoken_role()
    }

    /// Grants the specified PortfolioToken role to the user
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN role
    pub fn grant_portfolio_token_role(
        ctx: Context<PortfolioTokenGrantRole>,
        role: RoleType,
        user: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .grant_portfolio_token_role(role, user, &ctx.bumps)
    }

    /// Revokes the specified PortfolioToken role from the user
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN role
    pub fn revoke_portfolio_token_role(ctx: Context<PortfolioTokenRevokeRole>) -> Result<()> {
        ctx.accounts.revoke_portfolio_token_role()
    }

    /// Grants the specified GM Token Factory role to the user
    /// Signer must have the ADMIN_ROLE_GMTOKEN_FACTORY role
    pub fn grant_gmtoken_factory_role(
        ctx: Context<GMTokenFactoryGrantRole>,
        role: RoleType,
        user: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .grant_gmtoken_factory_role(role, user, &ctx.bumps)
    }

    /// Revokes the specified GM Token Factory role from the user
    /// Signer must have the ADMIN_ROLE_GMTOKEN_FACTORY role
    pub fn revoke_gmtoken_factory_role(ctx: Context<GMTokenFactoryRevokeRole>) -> Result<()> {
        ctx.accounts.revoke_gmtoken_factory_role()
    }

    /// Grants the specified Portfolio Token Factory role to the user
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_FACTORY role
    pub fn grant_portfolio_token_factory_role(
        ctx: Context<PortfolioTokenFactoryGrantRole>,
        role: RoleType,
        user: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .grant_portfolio_token_factory_role(role, user, &ctx.bumps)
    }

    /// Revokes the specified Portfolio Token Factory role from the user
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_FACTORY role
    pub fn revoke_portfolio_token_factory_role(
        ctx: Context<PortfolioTokenFactoryRevokeRole>,
    ) -> Result<()> {
        ctx.accounts.revoke_portfolio_token_factory_role()
    }

    /// Grants the specified Token Limit role to the user
    /// Signer must have the ADMIN_ROLE_TOKEN_LIMIT role
    pub fn grant_token_limit_role(
        ctx: Context<TokenLimitGrantRole>,
        role: RoleType,
        user: Pubkey,
    ) -> Result<()> {
        ctx.accounts.grant_token_limit_role(role, user, &ctx.bumps)
    }

    /// Revokes the specified Token Limit role from the user
    /// Signer must have the ADMIN_ROLE_TOKEN_LIMIT role
    pub fn revoke_token_limit_role(ctx: Context<TokenLimitRevokeRole>) -> Result<()> {
        ctx.accounts.revoke_token_limit_role()
    }

    // All Pause Controls
    // --------------------------------------------------------------------------------

    // Factory - 1 permissioned pause, 2 admin pause/resume

    /// Pause the GM token factory
    /// Signer must have the PAUSER_ROLE_GMTOKEN_FACTORY role
    pub fn pause_gmtoken_factory(ctx: Context<GMTokenFactoryPauser>) -> Result<()> {
        ctx.accounts.pause_factory()
    }

    /// Pause the GM token factory (admin version)
    /// Signer must have the ADMIN_ROLE_GMTOKEN_FACTORY role
    pub fn pause_gmtoken_factory_admin(ctx: Context<GMTokenFactoryAdmin>) -> Result<()> {
        ctx.accounts.pause_factory()
    }

    /// Resume the GM token factory
    /// Signer must have the ADMIN_ROLE_GMTOKEN_FACTORY role
    pub fn resume_gmtoken_factory(ctx: Context<GMTokenFactoryAdmin>) -> Result<()> {
        ctx.accounts.resume_factory()
    }

    /// Pause the Portfolio token factory
    /// Signer must have the PAUSER_ROLE_PORTFOLIO_TOKEN_FACTORY role
    pub fn pause_portfolio_token_factory(ctx: Context<PortfolioTokenFactoryPauser>) -> Result<()> {
        ctx.accounts.pause_factory()
    }

    /// Pause the Portfolio token factory (admin version)
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_FACTORY role
    pub fn pause_portfolio_token_factory_admin(
        ctx: Context<PortfolioTokenFactoryAdmin>,
    ) -> Result<()> {
        ctx.accounts.pause_factory()
    }

    /// Resume the Portfolio token factory
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_FACTORY role
    pub fn resume_portfolio_token_factory(ctx: Context<PortfolioTokenFactoryAdmin>) -> Result<()> {
        ctx.accounts.resume_factory()
    }

    // Token-2022 Pausable extension (per mint) - 1 permissioned pause, 1 permissioned resume

    /// Pause minting, burning, and transfers for a GM token via Token-2022's Pausable extension
    /// Signer must have the PAUSER_ROLE_GMTOKEN role
    pub fn pause_gmtoken(ctx: Context<PauseGMToken>) -> Result<()> {
        ctx.accounts.pause(ctx.bumps.mint_authority)
    }

    /// Clear a GM token's Token-2022 pause on minting, burning, and transfers
    /// Signer must have the UNPAUSER_ROLE_GMTOKEN role
    pub fn resume_gmtoken(ctx: Context<ResumeGMToken>) -> Result<()> {
        ctx.accounts.resume(ctx.bumps.mint_authority)
    }

    /// Pause minting, burning, and transfers for a Portfolio token via Token-2022's Pausable extension
    /// Signer must have the PAUSER_ROLE_PORTFOLIO_TOKEN role
    pub fn pause_portfolio_token(ctx: Context<PausePortfolioToken>) -> Result<()> {
        ctx.accounts.pause(ctx.bumps.mint_authority)
    }

    /// Clear a Portfolio token's Token-2022 pause on minting, burning, and transfers
    /// Signer must have the UNPAUSER_ROLE_PORTFOLIO_TOKEN role
    pub fn resume_portfolio_token(ctx: Context<ResumePortfolioToken>) -> Result<()> {
        ctx.accounts.resume(ctx.bumps.mint_authority)
    }

    // Global Mint - 1 permissioned pause, 2 admin pause/resume

    /// Pause GM mints globally
    /// Signer must have the PAUSER_ROLE_GMTOKEN_MANAGER role
    pub fn pause_global_gm_minting(ctx: Context<GMTokenManagerGlobalPauser>) -> Result<()> {
        ctx.accounts.pause_global_minting()
    }

    /// Resume GM mints globally
    /// Signer must have the ADMIN_ROLE_GMTOKEN_MANAGER role
    pub fn resume_global_gm_minting(ctx: Context<GMTokenManagerAdminPauser>) -> Result<()> {
        ctx.accounts.resume_global_minting()
    }

    /// Pause GM minting globally (admin function)
    /// Signer must have the ADMIN_ROLE_GMTOKEN_MANAGER role
    pub fn pause_global_gm_minting_admin(ctx: Context<GMTokenManagerAdminPauser>) -> Result<()> {
        ctx.accounts.pause_global_minting()
    }

    /// Pause Portfolio minting globally
    /// Signer must have the PAUSER_ROLE_PORTFOLIO_TOKEN_MANAGER role
    pub fn pause_portfolio_global_minting(
        ctx: Context<PortfolioTokenManagerGlobalPauser>,
    ) -> Result<()> {
        ctx.accounts.pause_global_minting()
    }

    /// Resume Portfolio minting globally
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER role
    pub fn resume_portfolio_global_minting(
        ctx: Context<PortfolioTokenManagerAdminPauser>,
    ) -> Result<()> {
        ctx.accounts.resume_global_minting()
    }

    /// Pause Portfolio minting globally (admin function)
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER role
    pub fn pause_portfolio_global_minting_admin(
        ctx: Context<PortfolioTokenManagerAdminPauser>,
    ) -> Result<()> {
        ctx.accounts.pause_global_minting()
    }

    // Global Redeem - 1 permissioned pause, 2 admin pause/resume

    /// Pause GM redemption globally
    /// Signer must have the PAUSER_ROLE_GMTOKEN_MANAGER role
    pub fn pause_global_gm_redemption(ctx: Context<GMTokenManagerGlobalPauser>) -> Result<()> {
        ctx.accounts.pause_global_redemption()
    }

    /// Resume GM redemption globally
    /// Signer must have the ADMIN_ROLE_GMTOKEN_MANAGER role
    pub fn resume_global_gm_redemption(ctx: Context<GMTokenManagerAdminPauser>) -> Result<()> {
        ctx.accounts.resume_global_redemption()
    }

    /// Pause GM redemption globally (admin function)
    /// Signer must have the ADMIN_ROLE_GMTOKEN_MANAGER role
    pub fn pause_global_gm_redemption_admin(ctx: Context<GMTokenManagerAdminPauser>) -> Result<()> {
        ctx.accounts.pause_global_redemption()
    }

    /// Pause Portfolio redemption globally
    /// Signer must have the PAUSER_ROLE_PORTFOLIO_TOKEN_MANAGER role
    pub fn pause_portfolio_global_redemption(
        ctx: Context<PortfolioTokenManagerGlobalPauser>,
    ) -> Result<()> {
        ctx.accounts.pause_global_redemption()
    }

    /// Resume Portfolio redemption globally
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER role
    pub fn resume_portfolio_global_redemption(
        ctx: Context<PortfolioTokenManagerAdminPauser>,
    ) -> Result<()> {
        ctx.accounts.resume_global_redemption()
    }

    /// Pause Portfolio redemption globally (admin function)
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER role
    pub fn pause_portfolio_global_redemption_admin(
        ctx: Context<PortfolioTokenManagerAdminPauser>,
    ) -> Result<()> {
        ctx.accounts.pause_global_redemption()
    }

    // Token Redemption - 1 permissioned pause, 2 admin pause/resume

    /// Pause redemptions for a specific token
    /// Signer must have the PAUSER_ROLE_TOKEN_LIMIT role
    pub fn pause_token_redemption(ctx: Context<TokenLimitPauser>) -> Result<()> {
        ctx.accounts.pause_token_redemption()
    }

    /// Resume redemptions for a token (admin function)
    /// Signer must have the ADMIN_ROLE_TOKEN_LIMIT role
    pub fn resume_token_redemption(ctx: Context<TokenLimitAdminPauser>) -> Result<()> {
        ctx.accounts.resume_token_redemption()
    }

    /// Pause redemptions for a specific token (admin function)
    /// Signer must have the ADMIN_ROLE_TOKEN_LIMIT role
    pub fn pause_token_redemption_admin(ctx: Context<TokenLimitAdminPauser>) -> Result<()> {
        ctx.accounts.pause_token_redemption()
    }

    // Token Minting - 1 permissioned pause, 2 admin pause/resume

    /// Pause minting for a specific token
    /// Signer must have the PAUSER_ROLE_TOKEN_LIMIT role
    pub fn pause_token_minting(ctx: Context<TokenLimitPauser>) -> Result<()> {
        ctx.accounts.pause_token_minting()
    }

    /// Resume mints for a token (admin function)
    /// Signer must have the ADMIN_ROLE_TOKEN_LIMIT role
    pub fn resume_token_minting(ctx: Context<TokenLimitAdminPauser>) -> Result<()> {
        ctx.accounts.resume_token_minting()
    }

    /// Pause minting for a specific token (admin function)
    /// Signer must have the ADMIN_ROLE_TOKEN_LIMIT role
    pub fn pause_token_minting_admin(ctx: Context<TokenLimitAdminPauser>) -> Result<()> {
        ctx.accounts.pause_token_minting()
    }

    // End Pause Controls
    // --------------------------------------------------------------------------------

    /// Set rate limit for a user
    /// Signer must have the ADMIN_ROLE_USER_LIMIT role
    pub fn set_ondo_user_limits(
        ctx: Context<SetUserLimits>,
        rate_limit: u64,
        limit_window: u64,
    ) -> Result<()> {
        ctx.accounts.set_ondo_user_limits(rate_limit, limit_window)
    }

    /// Revoke a role by closing the Roles account and reclaim rent
    /// Signer must be the upgrade authority of the program
    pub fn revoke_role(ctx: Context<RevokeRole>, _role: RoleType) -> Result<()> {
        ctx.accounts.revoke_role()
    }

    // For GM tokens (no permanent delegate)
    /// Initialize a new GM token mint (without permanent delegate)
    /// Signer must have the DEPLOYER_ROLE_TOKEN_FACTORY role
    pub fn init_mint(
        ctx: Context<TokenFactory>,
        name: String,
        symbol: String,
        uri: String,
        freeze_authority: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .init_mint(name, symbol, uri, freeze_authority, &ctx.bumps)
    }

    // For USDon (with permanent delegate)
    /// Initialize a new token mint with permanent delegate (for USDon)
    /// Signer must have the DEPLOYER_ROLE_TOKEN_FACTORY role
    pub fn init_mint_delegate(
        ctx: Context<TokenFactoryDelegate>,
        name: String,
        symbol: String,
        uri: String,
        freeze_authority: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .init_mint_delegate(name, symbol, uri, freeze_authority, &ctx.bumps)
    }

    // For Portfolio tokens (no permanent delegate)
    /// Initialize a new Portfolio token mint
    /// Signer must have the DEPLOYER_ROLE_TOKEN_FACTORY role
    pub fn init_portfolio_mint(
        ctx: Context<PortfolioTokenFactory>,
        name: String,
        symbol: String,
        uri: String,
        freeze_authority: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .init_mint(name, symbol, uri, freeze_authority, &ctx.bumps)
    }

    /// Update the secp256k1 attestation signer address
    /// Signer must have the ADMIN_ROLE_GMTOKEN_MANAGER role
    pub fn set_attestation_signer_secp(
        ctx: Context<GMTokenManagerSetSigner>,
        attestation_signer_secp: [u8; 20],
    ) -> Result<()> {
        ctx.accounts
            .set_attestation_signer_secp(attestation_signer_secp)
    }

    /// Update the secp256k1 attestation signer address for Portfolio tokens
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER role
    pub fn set_portfolio_attestation_signer_secp(
        ctx: Context<PortfolioTokenManagerSetSigner>,
        attestation_signer_secp: [u8; 20],
    ) -> Result<()> {
        ctx.accounts
            .set_attestation_signer_secp(attestation_signer_secp)
    }

    /// Grant a GM Token Manager role
    /// Signer must have the ADMIN_ROLE_GMTOKEN_MANAGER role
    pub fn grant_gmtoken_manager_role(
        ctx: Context<GMTokenManagerGrantRole>,
        role: RoleType,
        user: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .grant_gmtoken_manager_role(role, user, &ctx.bumps)
    }

    /// Revoke a role from the GM token manager
    /// Signer must have the ADMIN_ROLE_GMTOKEN_MANAGER role
    pub fn revoke_gmtoken_manager_role(ctx: Context<GMTokenManagerRevokeRole>) -> Result<()> {
        ctx.accounts.revoke_gmtoken_manager_role()
    }

    /// Grant a Portfolio Token Manager role
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER role
    pub fn grant_portfolio_token_manager_role(
        ctx: Context<PortfolioTokenManagerGrantRole>,
        role: RoleType,
        user: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .grant_portfolio_token_manager_role(role, user, &ctx.bumps)
    }

    /// Revoke a role from the Portfolio token manager
    /// Signer must have the ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER role
    pub fn revoke_portfolio_token_manager_role(
        ctx: Context<PortfolioTokenManagerRevokeRole>,
    ) -> Result<()> {
        ctx.accounts.revoke_portfolio_token_manager_role()
    }

    /// Grant a setter role for sanity check
    /// Signer must have the ADMIN_ROLE_ONDO_SANITY_CHECK role
    pub fn grant_sanity_setter_role(
        ctx: Context<OndoSanitySetterGrantRole>,
        role: RoleType,
        user: Pubkey,
    ) -> Result<()> {
        ctx.accounts.grant_setter_role(role, user, &ctx.bumps)
    }

    /// Revoke a setter role for sanity check
    /// Signer must have the ADMIN_ROLE_ONDO_SANITY_CHECK role
    pub fn revoke_sanity_setter_role(ctx: Context<OndoSanitySetterRevokeRole>) -> Result<()> {
        ctx.accounts.revoke_setter_role()
    }

    /// Update the last price in sanity check
    /// Signer must have the SETTER_ROLE_ONDO_SANITY_CHECK role
    pub fn set_last_price(ctx: Context<SetSanityCheck>, last_price: u64) -> Result<()> {
        ctx.accounts.set_last_price(last_price)
    }

    /// Grant a configurer role for sanity check
    /// Signer must have the ADMIN_ROLE_ONDO_SANITY_CHECK role
    pub fn grant_sanity_configurer_role(
        ctx: Context<OndoSanityConfigurerGrantRole>,
        role: RoleType,
        user: Pubkey,
    ) -> Result<()> {
        ctx.accounts.grant_configurer_role(role, user, &ctx.bumps)
    }

    /// Revoke a configurer role for sanity check
    /// Signer must have the ADMIN_ROLE_ONDO_SANITY_CHECK role
    pub fn revoke_sanity_configurer_role(
        ctx: Context<OndoSanityConfigurerRevokeRole>,
    ) -> Result<()> {
        ctx.accounts.revoke_configurer_role()
    }

    /// Set maximum time delay for sanity check
    /// Signer must have the CONFIGURER_ROLE_ONDO_SANITY_CHECK role
    pub fn set_max_time_delay(ctx: Context<ConfigSanityCheck>, max_time_delay: i64) -> Result<()> {
        ctx.accounts.set_max_time_delay(max_time_delay)
    }

    /// Set allowed price deviation in basis points
    /// Signer must have the CONFIGURER_ROLE_ONDO_SANITY_CHECK role
    pub fn set_allowed_deviation_bps(
        ctx: Context<ConfigSanityCheck>,
        allowed_deviation_bps: u64,
    ) -> Result<()> {
        ctx.accounts
            .set_allowed_deviation_bps(allowed_deviation_bps)
    }

    /// Update a GM token's UI multiplier for token display
    /// Signer must have the UPDATE_MULTIPLIER_ROLE role
    pub fn update_scaled_ui_multiplier(
        ctx: Context<UpdateScaledUiMultiplier>,
        new_multiplier: f64,
        timestamp: i64,
    ) -> Result<()> {
        ctx.accounts.update_scaled_ui_multiplier(
            new_multiplier,
            timestamp,
            ctx.bumps.mint_authority,
        )
    }

    /// Update a GM token's or USDon's metadata (name, symbol, URI)
    /// Signer must have the UPDATE_METADATA_ROLE role
    pub fn update_token_metadata(
        ctx: Context<UpdateGMTokenMetadata>,
        new_name: Option<String>,
        new_symbol: Option<String>,
        new_uri: Option<String>,
    ) -> Result<()> {
        ctx.accounts
            .update_token_metadata(new_name, new_symbol, new_uri, ctx.bumps)
    }

    /// Update a Portfolio token's metadata (name, symbol, URI)
    /// Signer must have the UPDATE_METADATA_ROLE role
    pub fn update_portfolio_token_metadata(
        ctx: Context<UpdatePortfolioTokenMetadata>,
        new_name: Option<String>,
        new_symbol: Option<String>,
        new_uri: Option<String>,
    ) -> Result<()> {
        ctx.accounts
            .update_token_metadata(new_name, new_symbol, new_uri, ctx.bumps)
    }

    /// Close a single attestation account
    ///
    /// The attestation account must be older than 30 seconds to be closed.
    /// The rent from the closed account is returned to the recipient (original creator).
    /// Unpermissioned
    pub fn close_attestation_account(
        ctx: Context<CloseAttestationAccount>,
        _attestation_id: [u8; 16],
    ) -> Result<()> {
        ctx.accounts.close_attestation_account()
    }

    /// Batch close attestation accounts
    ///
    /// Accounts to close are passed via remaining_accounts, constraints:
    /// 1. Accounts must be marked writable
    /// 2. No other accounts should present in `remaining_accounts`
    /// 3. Each attestation account must be created by the recipient
    /// 4. Each attestation must be older than 30 seconds
    /// Unpermissioned
    pub fn batch_close_attestation_accounts<'info>(
        ctx: Context<'_, '_, 'info, 'info, BatchCloseAttestationAccounts<'info>>,
    ) -> Result<()> {
        ctx.accounts
            .batch_close_attestation_accounts(ctx.remaining_accounts)
    }
}
