use anchor_lang::prelude::*;

#[error_code]
pub enum OndoError {
    #[msg("Invalid Input Mint")]
    InvalidInputMint,
    #[msg("Invalid Amount")]
    InvalidAmount,
    #[msg("Invalid Price")]
    InvalidPrice,
    #[msg("Invalid Rate Limit")]
    InvalidRateLimit,
    #[msg("Invalid Mints")]
    InvalidMints,
    #[msg("Invalid Token Account")]
    InvalidTokenAccount,
    #[msg("WARNING: USDC Below Minimum Price")]
    USDCBelowMinimumPrice,
    #[msg("Math Overflow")]
    MathOverflow,
    #[msg("ProgramMismatch")]
    ProgramMismatch,
    #[msg("DataMismatch")]
    DataMismatch,
    #[msg("Address not found in the specified role")]
    AddressNotFoundInRole,
    #[msg("Invalid User")]
    InvalidUser,
    #[msg("Invalid Attestation")]
    AttestationExpired,
    #[msg("Attestation expiration time too large")]
    AttestationExpirationTooLarge,
    #[msg("Invalid Role Type")]
    InvalidRoleType,
    #[msg("GMToken Factory Paused")]
    GMTokenFactoryPaused,
    #[msg("GMToken Redemption Paused")]
    GMTokenRedemptionPaused,
    #[msg("GMToken Minting Paused")]
    GMTokenMintingPaused,
    #[msg("User is not whitelisted")]
    UserNotWhitelisted,
    #[msg("Price exceeds maximum allowed deviation")]
    PriceExceedsMaxDeviation,
    #[msg("Price below minimum allowed deviation")]
    PriceBelowMinDeviation,
    #[msg("Vault address not valid")]
    InvalidVault,
    #[msg("Invalid sanity check percentage")]
    InvalidPercentage,
    #[msg("Invalid sanity check max_time_delay")]
    InvalidMaxTimeDelay,
    #[msg("Sanity check time expired")]
    MaxTimeDelayExceeded,
    #[msg("Attestation signer Ethereum address not set")]
    AttestationSignerEthAddressNotSet,
    #[msg("Attestation is too new to be closed")]
    AttestationTooNew,
    #[msg("Attestation already used")]
    AttestationAlreadyUsed,
    #[msg("Token program not provided")]
    TokenProgramNotProvided,
    #[msg("Divide by zero")]
    DivideByZero,
    #[msg("Invalid oracle price address provided")]
    InvalidOraclePriceAddress,
    #[msg("Invalid oracle price max age provided")]
    InvalidOraclePriceMaxAge,
    #[msg("USDC price oracle was not provided for USDC swap")]
    USDCOracleNotProvided,
    #[msg("The provided USDC price oracle is not implemented")]
    USDCOracleNotImplemented,
    #[msg("Maximum timezone offset exceeded")]
    MaximumOffsetExceeded,
    #[msg("Trade attempted outside market hours")]
    OutsideMarketHours,
    #[msg("Mint must have a freeze authority or have the permanent delegate extension enabled")]
    InvalidMintConfiguration,
    #[msg("Confidence threshold exceeded")]
    ConfidenceThresholdExceeded,
    #[msg("Invalid price exponent")]
    InvalidPriceExponent,
    #[msg("A required mint was not provided")]
    MintNotProvided,
    #[msg("Amount exceeds maximum mint amount")]
    AmountExceedsMaxMintAmount,
    #[msg("No metadata fields to update")]
    NoMetadataFieldsToUpdate,
    #[msg("Metadata field too long")]
    MetadataFieldTooLong,
    #[msg("Time since last update has a negative value")]
    NegativeTimeSinceLastUpdate,
}
