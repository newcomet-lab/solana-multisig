#!/bin/bash
set -xeuo pipefail

GROUP=$(cat config.txt | cut -d ' ' -f2)
PROT=$(cat config.txt | cut -d ' ' -f3)

npm run build:program
BUFFER=$(solana program write-buffer  /home/oak/projects/solana/solana_multisig/dist/program/solana_multisig.so | cut -d ' ' -f2)

solana program set-buffer-authority --new-buffer-authority $PROT $BUFFER

PROPOSAL=$(npm run start propose -- upgrade-multisig --group $GROUP --key ./keys/alpha.json --buffer $BUFFER | rg -F 'public key: ' | cut -d ' ' -f8)

npm run start approve -- --key ./keys/beta.json --proposal $PROPOSAL