#!/usr/bin/env sh
# Dump the Metaplex Token Metadata program from devnet for the LiteSVM tests.
# Read-only RPC call; the .so is not committed (see .gitignore).
set -eu
cd "$(dirname "$0")/.."
mkdir -p tests/fixtures
solana program dump -u devnet metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s tests/fixtures/mpl_token_metadata.so
