# token-nft-mint

> **Not audited. Devnet / localnet only.** Learning project: do not deploy to mainnet or use with real funds.

An Anchor 1.2 program that creates:

1. a **fungible SPL token** with Metaplex Token Metadata (name, symbol, URI), and mints it to any
   wallet;
2. a **1-of-1 NFT**: a 0-decimal mint, exactly one token minted to the creator, a metadata
   account and a **master edition** with `max_supply = 0`.

It also includes a TypeScript script that calls both on devnet or localnet.

## How the NFT works

```
create_nft:
  init mint (decimals 0, authority = payer)        [Anchor `init` + `mint::*` constraints]
  init payer's ATA                                  [associated_token::*]
  mint_to(1)                                        [CPI SPL Token]
  create_metadata_accounts_v3                       [CPI Token Metadata]
  create_master_edition_v3(max_supply = Some(0))    [CPI Token Metadata]
```

Creating the master edition **moves the mint and freeze authority to the edition PDA**. After
that, nobody can mint a second token. The tests check this: `mint_authority == edition PDA`, and
a later `mint_tokens` fails with `ConstraintMintMintAuthority`.

Metadata and edition addresses are Token Metadata PDAs
(`["metadata", program, mint]` and `[..., "edition"]`). The program checks them with
`seeds::program = token_metadata_program`, so a fake metadata account is rejected.

## Quick start

```bash
anchor build                      # or: anchor build --tools-version v1.58
./scripts/fetch-programs.sh       # dumps Token Metadata from devnet into tests/fixtures (not committed)
cargo test                        # LiteSVM with the real Token Metadata program loaded

pnpm install
pnpm mint token --name "Test Dinar" --symbol TDZD --uri https://example.org/tdzd.json --decimals 6 --amount 1000 --cluster devnet
pnpm mint nft   --name "Sahara #1" --symbol ART --uri https://example.org/nft/1.json --cluster devnet
```

The script was run end to end against `solana-test-validator`, with the program and the dumped
Token Metadata program loaded through `--bpf-program`. It created a token and an NFT.

The script reads the program ID and discriminators from `target/idl/token_nft_mint.json`. It refuses
mainnet, both by URL and by genesis hash, and never prints the wallet's secret key.

## Tests (`programs/token-nft-mint/tests/mint.rs`)

- `fungible_token_with_metadata`: metadata fields and authority; minting twice adds up
- `only_mint_authority_can_mint`: **wrong signer** (`mint::authority`)
- `metadata_length_limits`: name > 32 or symbol > 10 bytes rejected with custom errors
- `nft_is_one_of_one_with_master_edition`: supply 1, master edition `max_supply = Some(0)`, mint
  authority moved to the edition, and further minting rejected
- `nft_requires_mint_keypair_signature`: `init` of a new mint needs the mint's own signature

## Status / limitations

- No collections or verified creators. Royalties are set to 0.
- Fungible metadata uses `create_metadata_accounts_v3` (not the newer `CreateV1` / programmable NFTs).
- Tests need the dumped Token Metadata binary. Without it they fail with a message telling you to
  run `scripts/fetch-programs.sh`.

## License

MIT © Abdeldjalil Chougui
