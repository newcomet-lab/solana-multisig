#!/bin/bash
set -xeuo pipefail

GROUP=$(cat config.txt | cut -d ' ' -f2)
PROT=$(cat config.txt | cut -d ' ' -f3)

DELTA=$(solana-keygen pubkey keys/delta.json)

PROPOSAL=$(npm run start propose -- transfer --group $GROUP --key ./keys/alpha.json --destination $(solana-keygen pubkey ./keys/alpha.json) --lamports 1002 | rg -F 'public key: ' |  cut -d ' ' -f8)

solana balance $DELTA
npm run start close-proposal -- --key ./keys/alpha.json --proposal $PROPOSAL --destination $DELTA
solana balance $DELTA