# Ondo Global Markets - Solana Program

A Solana smart contract implementing Ondo Finance's Global Markets (GM) token infrastructure. This program enables the creation, minting, and redemption of GM tokens with comprehensive access control, rate limiting, and oracle integration.

## License

This project is licensed under the Business Source License 1.1 (BUSL-1.1). See [LICENSE](LICENSE.md) for details.

## Overview

The Ondo Global Markets program is built on Solana using the Anchor framework and leverages Token-2022 extensions for advanced token functionality. It provides:

- **Token Factory**: Deploy new GM tokens with configurable extensions
- **Mint/Redeem Operations**: Exchange GM tokens for stablecoins with attestation-based verification
- **Role-Based Access Control**: Granular permission system for administrative operations
- **Rate Limiting**: Token-level and user-level rate limits with time-windowed capacity decay
- **Oracle Integration**: Price validation via Pyth oracle feeds with sanity checks
- **Pause Controls**: Multi-layered pause system for risk management

## Architecture

### Core Components

| Component | Description |
|-----------|-------------|
| `GMTokenManagerState` | Global program configuration including pause states and attestation signer |
| `USDonManagerState` | USDon mint and vault configuration with USDC oracle settings |
| `TokenLimit` | Per-token rate limits and pause flags |
| `OndoUser` | Per-user rate limit tracking for minting and redemption |
| `Whitelist` | Access control for swap operations |
| `Attestation` | Single-use attestation accounts for replay protection |

### Token Extensions

GM tokens are created with the following Token-2022 extensions:

- **ScaledUiAmount** - Display scaling for token amounts
- **MetadataPointer** - On-chain token metadata
- **Pausable** - Global transfer pause capability
- **ConfidentialTransferMint** - Confidential transfer support
- **TransferHook** - Custom transfer logic hooks

### Role System

The program implements a comprehensive role-based access control system with distinct roles for:

- Token factory administration and deployment
- Minting and burning operations
- Pause/unpause controls at multiple levels
- Oracle sanity check configuration
- Whitelist management
- Metadata updates

## Building

### Prerequisites

- Rust 1.75+
- Solana CLI 1.18+
- Anchor CLI 0.32+

### Build Commands

```bash
# Build for localnet (default)
anchor build

# Build for specific network
anchor build -- --no-default-features --features mainnet
anchor build -- --no-default-features --features devnet
anchor build -- --no-default-features --features testnet
```

### Network Features

The program uses compile-time features to configure network-specific constants:

| Feature | Description |
|---------|-------------|
| `localnet` | Local development (default) |
| `devnet` | Solana Devnet |
| `testnet` | Solana Testnet |
| `mainnet` | Solana Mainnet |

## Testing

```bash
# Run all tests
anchor test

# Run tests without rebuilding
anchor test --skip-build
```

## Security

### Attestation Verification

All mint and redeem operations require a signed attestation verified via secp256k1 signature recovery. The attestation hash includes:

- Chain ID
- Attestation ID
- Operation side (buy/sell)
- User address
- Asset
- Price
- Amount
- Expiration

### Rate Limiting

Two-tier rate limiting system:

1. **Token-level**: Global caps on mint/redeem volume per time window
2. **User-level**: Per-user caps with configurable defaults

Capacity decays over time based on the configured window size.

### Oracle Sanity Checks

Price feeds are validated against:

- Maximum allowed deviation from last known good price
- Maximum staleness threshold

### Security Contact

For security concerns, please contact: security@ondo.finance

Bug bounty program: https://immunefi.com/bug-bounty/ondofinance/information/

### Audits

Audit reports are available at: https://docs.ondo.finance/audits

## Dependencies

| Dependency | Version |
|------------|---------|
| anchor-lang | 0.32.1 |
| anchor-spl | 0.32.1 |
| spl-token-2022 | 8.0.0 |
| pyth-solana-receiver-sdk | 0.6.1 |

## Resources

- [Ondo Finance](https://ondo.finance)
- [Documentation](https://docs.ondo.finance)
