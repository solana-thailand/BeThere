# BeThere cNFT Tooling

Rust CLI (`bethere-cnft`) for managing compressed NFTs on Solana devnet using **Bubblegum V2** and **MPL Account Compression**.

## Setup

### 1. Generate a keypair

```bash
# Option A: Use solana-keygen (recommended)
solana-keygen new --outfile keys/payer.json

# Option B: Generate with the CLI
cd scripts/cnft
cargo run -- --help
```

### 2. Fund on devnet

```bash
cargo run -- airdrop
# Requests 2 SOL on devnet
```

### 3. Create a Merkle tree

```bash
# Small tree (16K capacity, ~0.34 SOL)
cargo run -- create-tree --preset small

# Medium tree (131K capacity, ~1.4 SOL)
cargo run -- create-tree --preset medium --label "Event #1"

# Custom tree
cargo run -- create-tree --depth 17 --buffer 64
```

### 4. Mint a cNFT

```bash
# Mint to yourself
cargo run -- mint --tree <TREE_ADDRESS>

# Mint to a specific wallet
cargo run -- mint --tree <TREE_ADDRESS> --recipient <WALLET> --name "POAP #1"

# Use tree label from local registry
cargo run -- mint --label "Event #1" --recipient <WALLET>
```

### 5. Check tree info

```bash
cargo run -- tree-info --tree <TREE_ADDRESS>
cargo run -- tree-info --label "Event #1"
```

## Commands

| Command | Description |
|---------|-------------|
| `airdrop` | Request 2 SOL on devnet |
| `balance` | Check SOL balance |
| `create-tree` | Create a Bubblegum V2 Merkle tree |
| `mint` | Mint a compressed NFT using `mint_v2` |
| `tree-info` | Show tree config and mint count |

## Program IDs (V2)

| Program | Address | Note |
|---------|---------|------|
| Bubblegum | `BGUMAp9Gq7iTEuizy4pqaxsTyUCBK68MDfK752saRPUY` | cNFT program |
| MPL Account Compression | `mcmt6YrQEMKw8Mw43FmpRLmf7BqRnFMKmAcbxE3xkAW` | V2 compression (not legacy SPL) |
| MPL Noop | `mnoopTCrg4p8ry25e4bcWA9XZjbNjMTfgYVGGEdRsf3` | V2 noop (not legacy) |
| MPL Core | `CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d` | Required by `mint_v2` |

> **Important:** Legacy SPL Account Compression (`cmtDvXum...`) and legacy Noop (`noopb9bk...`) are **not compatible** with Bubblegum V2 trees. Always use the MPL program IDs above.

## Tree Presets

| Preset | Depth | Buffer | Capacity | Approx Cost |
|--------|-------|--------|----------|-------------|
| small | 14 | 64 | 16,384 | ~0.34 SOL |
| medium | 17 | 64 | 131,072 | ~1.4 SOL |
| large | 20 | 1024 | 1,048,576 | ~8.5 SOL |

## Mint Options

```
cargo run -- mint [OPTIONS]

Options:
  --tree <ADDRESS>      Tree address (required unless --label is set)
  --label <NAME>        Tree label from local registry
  --recipient <WALLET>  Recipient wallet (defaults to authority keypair)
  --name <TEXT>         NFT name
  --symbol <TEXT>       NFT symbol
  --uri <URL>           Metadata URI
```

## Architecture

```
scripts/cnft/
├── Cargo.toml              # bethere-cnft binary (Solana SDK 1.18, mpl-bubblegum 1.4, borsh 0.10)
├── keys/
│   ├── payer.json          # Authority keypair (Solana CLI format)
│   └── trees.json          # Local tree registry (auto-generated)
└── src/
    ├── main.rs             # CLI entry point with clap subcommands
    ├── config.rs           # Config, keypair loading, tree registry, V2 program IDs
    └── commands/
        ├── airdrop.rs      # Devnet SOL faucet
        ├── balance.rs      # Balance check
        ├── create_tree.rs  # CreateTreeV2 instruction (manual discriminator + account alloc)
        ├── mint.rs         # MintV2 instruction (manual Borsh serialization)
        └── tree_info.rs    # Read TreeConfig account
```

## Key Technical Details

### Manual Borsh Serialization

The `mint` command builds the `mint_v2` instruction manually rather than using the `mpl-bubblegum` SDK builder. This avoids version mismatch issues between the SDK and the on-chain program.

`MetadataArgsV2` is serialized in exact IDL field order:
1. `name` (String: 4-byte len + utf8)
2. `symbol` (String)
3. `uri` (String)
4. `seller_fee_basis_points` (u16)
5. `primary_sale_happened` (bool)
6. `is_mutable` (bool)
7. `token_standard` (Option<u8> — **u8, not u32**)
8. `creators` (Vec<CreatorArgs>)
9. `collection` (Option<Pubkey>)

### TokenStandard Bug

`TokenStandard` is an Anchor enum serialized as **u8** (0=NonFungible, 1=FungibleAsset, 2=Fungible, 3=NonFungibleEdition). Serializing it as u32 causes `InstructionDidNotDeserialize` on-chain.

### 13-Account Layout for mint_v2

| Index | Account | Writable | Signer | Notes |
|-------|---------|----------|--------|-------|
| 0 | treeAuthority | ✅ | — | PDA derived from merkle tree |
| 1 | payer | ✅ | ✅ | Pays for the mint |
| 2 | treeDelegate | — | ✅ | Defaults to payer |
| 3 | collectionAuthority | — | ✅ | Optional — program ID placeholder |
| 4 | leafOwner | — | — | NFT recipient |
| 5 | leafDelegate | — | — | Defaults to leafOwner |
| 6 | merkleTree | ✅ | — | The tree account |
| 7 | coreCollection | ✅ | — | Optional — program ID placeholder |
| 8 | mplCoreCpiSigner | — | — | Optional — program ID placeholder |
| 9 | logWrapper | — | — | MPL Noop |
| 10 | compressionProgram | — | — | MPL Account Compression |
| 11 | mplCoreProgram | — | — | Required even without collections |
| 12 | systemProgram | — | — | System program |

Optional accounts use the Bubblegum program ID as a placeholder.

## Verified Devnet Tree

| Field | Value |
|-------|-------|
| Address | `32xkLNBQELyMbynubjLUy665uZLFiSH9yFeiUWYU7ozw` |
| Network | devnet |
| Status | 3 successful mints confirmed |

## Worker Integration

The **Cloudflare Worker** uses Helius `mintCompressedNft` RPC for production minting — it does not call this CLI directly.

This CLI serves two purposes:
- **Tree management** — Creating and inspecting Merkle trees on devnet/mainnet
- **Testing** — Manual minting to verify tree configuration before production use

Production flow:
1. Create tree via CLI (`create-tree`)
2. Mint via Helius RPC (Worker) or CLI (testing)
3. Monitor via `tree-info`

## Security

- **Never commit `keys/` directory** — contains private keys
- `keys/.gitignore` prevents accidental commits
- Use a dedicated keypair for devnet testing only
