#!/bin/bash
set -xeuo pipefail

GROUP=$(cat config.txt | cut -d ' ' -f2)
PROT=$(cat config.txt | cut -d ' ' -f3)

MINT=$(spl-token create-token --mint-authority $PROT | rg 'Creating token' | cut -d ' ' -f3)

npm run start propose -- create-token-account --group $GROUP --key ./keys/alpha.json --mint $MINT --seed example1 > /tmp/creat-tok-tmp
cat /tmp/creat-tok-tmp | rg 'public key:' > /tmp/proposal-tmp
PROPOSAL=$(cat /tmp/proposal-tmp | cut -d ' ' -f8)
npm run start approve -- --key ./keys/beta.json --proposal $PROPOSAL
TOKEN_ACC1=$(cat /tmp/creat-tok-tmp | rg 'creating token account' | cut -d ' ' -f5)

npm run start propose -- create-token-account --group $GROUP --key ./keys/alpha.json --mint $MINT --seed example2 > /tmp/creat-tok-tmp
cat /tmp/creat-tok-tmp | rg 'public key:' > /tmp/proposal-tmp
PROPOSAL=$(cat /tmp/proposal-tmp | cut -d ' ' -f8)
npm run start approve -- --key ./keys/beta.json --proposal $PROPOSAL
TOKEN_ACC2=$(cat /tmp/creat-tok-tmp | rg 'creating token account' | cut -d ' ' -f5)

PROPOSAL=$(npm run start propose -- mint-to --group $GROUP --key ./keys/alpha.json --mint $MINT --destination $TOKEN_ACC1 --amount 1000000000 | rg 'key:' | xargs | cut -d ' ' -f8)
npm run start approve -- --key ./keys/beta.json --proposal $PROPOSAL
echo 'Before transfer'
spl-token supply $MINT
spl-token balance --address $TOKEN_ACC1

PROPOSAL=$(npm run start propose -- transfer-token --group $GROUP --key ./keys/alpha.json --amount 500000000 --destination $TOKEN_ACC2 --source $TOKEN_ACC1 | rg 'key:' | xargs | cut -d ' ' -f8)
npm run start approve -- --key ./keys/beta.json --proposal $PROPOSAL
echo 'After transfer'
spl-token supply $MINT
spl-token balance --address $TOKEN_ACC1
spl-token balance --address $TOKEN_ACC2