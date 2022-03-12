#!/bin/bash
set -xeuo pipefail

URL=${RPC_URL:-http://localhost:8899}
solana config set --url $URL
solana airdrop 5

ALPHA=$(solana-keygen pubkey keys/alpha.json)
BETA=$(solana-keygen pubkey keys/beta.json)
DELTA=$(solana-keygen pubkey keys/delta.json)
solana airdrop 5 $ALPHA
solana airdrop 5 $BETA
solana airdrop 5 $DELTA

npm run build:program
PROGRAM=$(npm run start deploy -- --authority keys/alpha.json | tee /dev/tty | rg 'Program deployed to account' | cut -d ' ' -f5)

npm run start init -- --threshold 2 $ALPHA:1,$BETA:1 --payer ./keys/alpha.json --create-protected | rg -F 'account:' > /tmp/set-multi-tmp

GROUP=$(rg 'group account: '  /tmp/set-multi-tmp | cut -d ' ' -f3)
PROT=$(rg 'protected account: ' /tmp/set-multi-tmp | cut -d ' ' -f3)
echo "$PROGRAM $GROUP $PROT" > config.txt
solana airdrop 5 $PROT

solana program set-upgrade-authority -k keys/alpha.json --new-upgrade-authority $PROT $PROGRAM

echo "Program: $PROGRAM"
echo "Group: $GROUP"
echo "Protected: $PROT"