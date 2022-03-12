#!/bin/bash
set -xeuo pipefail

GROUP=$(cat config.txt | cut -d ' ' -f2)
PROT=$(cat config.txt | cut -d ' ' -f3)


PROPOSAL=$(npm run start propose -- transfer --group $GROUP --key ./keys/alpha.json --destination $(solana-keygen pubkey ./keys/alpha.json) --lamports 1001 | rg -F 'public key: ' |  cut -d ' ' -f8)

npm run start approve -- --key ./keys/beta.json --proposal $PROPOSAL