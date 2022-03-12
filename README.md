# Multisig for Solana

## Setting up the environment

The following dependencies are required to build and run this example, depending
on your OS, they may already be installed:

- Install node
- Install npm
- Install the latest Rust stable from https://rustup.rs/
- Install Solana v1.5.8 or later from
  https://docs.solana.com/cli/install-solana-cli-tools

You may need to add to PATH cargo and solana-release binaries' locations:

- ~/.cargo/bin
- ~/.local/share/solana/install/releases/1.5.8/solana-release/bin

This example connects to a local Solana cluster by default.

Enable on-chain program logs:
```bash
$ export RUST_LOG=solana_runtime::system_instruction_processor=trace,solana_runtime::message_processor=debug,solana_bpf_loader=debug,solana_rbpf=debug
```

Start a local Solana cluster:
```bash
$ solana-test-validator --log
```

Install npm dependencies:
```bash
$ npm install
```

Set `RPC_URL` environment variable to a desired Solana cluster (e.g. `https://api.devnet.solana.com`).

## Creating keypairs

Create some accounts:

```bash
solana-keygen new --outfile ~/my-solana-wallet/alpha.json
```

The following keys are used in this example:
```
$ solana-keygen pubkey ~/my-solana-wallet/alpha.json
DmXqLX3WHZkF6Mvn6Ur6ryefbmRcTxMPn47xtquR66xY
$ solana-keygen pubkey ~/my-solana-wallet/beta.json
HbiWjapmq1ppw3ABsvfSd4fTbxs3MfNSkwPjcefiC9jw
$ solana-keygen pubkey ~/my-solana-wallet/gamma.json
95VQGQyvUy8Fwh7Kb2kyoCzwxiawsU8AdLopKju8CKV9
$ solana-keygen pubkey ~/my-solana-wallet/delta.json
79RnXYDieCofdT8N8TsQm368cZc1GqJ87xUJjSVaGNog
```
In our example, alpha, beta, and gamma are members of a multisig group, and delta will receive payment from them.

Airdrop some lamports on each account:
```
solana --url http://127.0.0.1:8899 airdrop 10 DmXqLX3WHZkF6Mvn6Ur6ryefbmRcTxMPn47xtquR66xY
```
Testnet and Devnet have airdrop restrictions - 1 SOL per request

## Building the program

```
$ npm run build:program
```

## Available operations

- `deploy` is used to deploy the multisig for the first time. 
- `init` is used to initialize a group account. The example is provided in [Initializing group account].
- `propose` is used to propose an instruction that can be executed on behalf of the group. There are several supported proposal types and they are listed below in [Available proposal types]
- `approve` is used to approve a proposal. When threshold of votes is reached, proposal will be executed. Arguments: `--proposal <proposalPubKey> --key <signerKeyPair>`.
- `multi-approve` can be used to executed several proposals in one transaction. For example it is used to create a new token account as 'account creation' and 'token account initialization' are two instruction that should be executed in one transaction. Arguments: `--proposals <proposalPubKey1,propodalPubKey2,..> --key <signerKeyPair>`.

## Available proposal types

- `create`
- `transfer`
- `upgrade`
- `upgrade-multisig`
- `mint-to`
- `create-token-account`
- `transfer-token`

## Program deployment

```
npm run start deploy -- --authority  ~/my-solana-wallet/alpha.json
...
Program deployed to account Zo9q743EQ5Zyr4GdsaLnLTYzG2xw251HdQ4UkHf4gK48
...
```

## Initializing group account

Specify group configuration and initialize a group:
```
$ npm run start init -- --threshold 2 DmXqLX3WHZkF6Mvn6Ur6ryefbmRcTxMPn47xtquR66xY:1,HbiWjapmq1ppw3ABsvfSd4fTbxs3MfNSkwPjcefiC9jw:1,95VQGQyvUy8Fwh7Kb2kyoCzwxiawsU8AdLopKju8CKV9:2 --payer ~/my-solana-wallet/delta.json --create-protected
...
created group account: 6KaBmpQuu8Yexwp4B8n2nhKAY6ZWncYPed8Az1NGjbw5
created protected account: 4ARCh8hB2WtsrgLSt1yhMNVjApzectwXehFVCVGy89WJ
...
```

You can also set program's upgrade authority to be this group's protected account in order to be able to upgrade multisig through multisig. (Values provided are examples)

```
solana program set-upgrade-authority -k ~/my-solana-wallet/alpha.json --new-upgrade-authority <protected account> <program_id>
```
Example:
```
solana program set-upgrade-authority -k ~/my-solana-wallet/alpha.json --new-upgrade-authority 4ARCh8hB2WtsrgLSt1yhMNVjApzectwXehFVCVGy89WJ Zo9q743EQ5Zyr4GdsaLnLTYzG2xw251HdQ4UkHf4gK48
```


#### Creating protected account

To create protected account during group initialization one must specify `--create-protected` flag
when calling init instruction. By default this will create protected account with 0 space allocated,
owned by a system program and a minimum amount of lamports sufficient for the account to be rent exempt.

*Please note that this amount will be charged from the initializer's account.*

One can override these defaults by providing one or more of the following arguments:
- `--protected-lamports <amount>`
- `--protected-owner <owner_pubkey>`
- `--protected-space <space>`

Please note that without `--create-protected` flag these arguments will have no effect.

## Proposing transfer

Alpha proposes a transfer from the protected account to delta's account:

```
$ npm run start -- propose transfer --group 6KaBmpQuu8Yexwp4B8n2nhKAY6ZWncYPed8Az1NGjbw5 --destination 79RnXYDieCofdT8N8TsQm368cZc1GqJ87xUJjSVaGNog --lamports 500000 --key ~/my-solana-wallet/alpha.json
...
created proposal account: 5dfvhauJwGX9XmNR2GRw3Zwch9byqiYcGEojjSUZTaMR
...
```

## Approving transfer

Beta approves the transfer:
```
$ npm run start -- approve --proposal 5dfvhauJwGX9XmNR2GRw3Zwch9byqiYcGEojjSUZTaMR --key ~/my-solana-wallet/beta.json
```

## Flow for Neon tokens

- Initialize keypairs as described in `Creating keypairs`. 
- Deploy the program as described in `Program deployment`. Program ID is used to send instructions to.
- Create group account as described in `Initializing group account`. Group account is used to derive group protected account's address. Group protected account is used to issue token related operations. This example will use these values:
```
group account: D6oqMgfyod8sivM2aa2WbfyMm4jMs6TzDmSSuWomMrUt
protected account: Btwu4MNoLTLTL6DSbkbXz5xdJyWkZZE5SuiDb5W9gruY
```
- Create token `spl-token create token --mint-authority Btwu4MNoLTLTL6DSbkbXz5xdJyWkZZE5SuiDb5W9gruY`. Mint in this example will be `FJohBKsdLXKcnbxXYxLfb2JmsAmbgTyvg9AeNv4pLPoU`.
- Create a token account `npm run start propose -- create-token-account --group D6oqMgfyod8sivM2aa2WbfyMm4jMs6TzDmSSuWomMrUt  --key ~/my-solana-wallet/alpha.json --mint FJohBKsdLXKcnbxXYxLfb2JmsAmbgTyvg9AeNv4pLPoU --seed example`. Seed is used to derive new account's address based on group protected account's address (createAccountWithSeed). Key will be used to sign proposal instruction and proposal account. This operation will create two proposal that must be approved simultaneously. Proposal pubkeys in this example will be:
```
created proposal accounts:
        key: HvTxwq1Bn6RDLKJhQrxxUi9LcL7BCVoTdsY9RtxNVPiU
        key: B1YWsqpUdvMGvfdNKU3MB2nszBnohERpeuxPa2EuChqe
```
This operation will also otput resulting token account's address. In this example it is `creating token account:  Hi8NFABcURggCJoL6hb9AeffCepY3VZKBHame1g5HNZ8`.
- Approve both proposals with the second account `npm run start multi-approve -- --key ~/my-solana-wallet/beta.json --proposals HvTxwq1Bn6RDLKJhQrxxUi9LcL7BCVoTdsY9RtxNVPiU,B1YWsqpUdvMGvfdNKU3MB2nszBnohERpeuxPa2EuChqe`
- Propose a minting operation  `npm run start propose -- mint-to --group D6oqMgfyod8sivM2aa2WbfyMm4jMs6TzDmSSuWomMrUt  --key ~/my-solana-wallet/alpha.json --mint FJohBKsdLXKcnbxXYxLfb2JmsAmbgTyvg9AeNv4pLPoU --destination Hi8NFABcURggCJoL6hb9AeffCepY3VZKBHame1g5HNZ8 --amount 100`.
- Approve it with the second account `npm run start approve -- --key ~/my-solana-wallet/beta.json --proposal 9ZJ2pR1J8KJu6UthCmpJYr6rbMAs8LNEqpjM8QkdFUGQ`.
- You can check that the token's supply is increased `spl-token supply FJohBKsdLXKcnbxXYxLfb2JmsAmbgTyvg9AeNv4pLPoU`.
- You can create a second token account as explained above. In this example this second account's address will be `JnbJr5sT3LHYyXZjKX7FUgrcH37ZSPkqHXKiwKQtkqz`.
- Propose a transferral operation `npm run start propose -- transfer-token --group D6oqMgfyod8sivM2aa2WbfyMm4jMs6TzDmSSuWomMrUt  --key ~/my-solana-wallet/alpha.json --source Hi8NFABcURggCJoL6hb9AeffCepY3VZKBHame1g5HNZ8 --destination JnbJr5sT3LHYyXZjKX7FUgrcH37ZSPkqHXKiwKQtkqz --amount 1` and approve it as described above.
- You can check both token accounts' balance with `spl-token balance --address <address>`

## Delegating a minting authority

One can get minting authority from multisig back with a following proposal: `npm run start propose -- delegate-mint-authority --group <GroupKey>  --key ./keys/alpha.json --target <MintKey> --new <NewAuthorityKey>`.


## Upgrading multisig

If an upgrade authority is delegated to a group's protected account you can upgrade multisig through multisig.

`Btwu4MNoLTLTL6DSbkbXz5xdJyWkZZE5SuiDb5W9gruY` is used as a group's protected account in this example.

- Build a program as described in `Building the program`
- Write a resulting binary to a buffer `solana program write-buffer <../>solana_multisig.so`. Buffer address in this example is `FMcFQnHvZjLyqaBPpEU8CxVqBX87WmvjNv5vYAtjZcPH`.
- Set a buffer authority to group's protected account address `solana program set-buffer-authority --new-buffer-authority Btwu4MNoLTLTL6DSbkbXz5xdJyWkZZE5SuiDb5W9gruY FMcFQnHvZjLyqaBPpEU8CxVqBX87WmvjNv5vYAtjZcPH`
- Propose multisig upgrade `npm run start propose -- upgrade-multisig --group D6oqMgfyod8sivM2aa2WbfyMm4jMs6TzDmSSuWomMrUt  --key ~/my-solana-wallet/alpha.json --buffer FMcFQnHvZjLyqaBPpEU8CxVqBX87WmvjNv5vYAtjZcPH`. Prposal account in this example is `E8C2W9UnGvvqTcvcU3k4SzApkAd3XTro1WLkY6qBNzT9`.
- Approve this upgrade `npm run start approve -- --key ~/my-solana-wallet/beta.json --proposal E8C2W9UnGvvqTcvcU3k4SzApkAd3XTro1WLkY6qBNzT`.
- You can check that the program has different properties now with `solana program show <ProgramID>`.

## Upgrading any other program

Assuming that the group's protected account has an upgrade authority over the specific program you can upgrade it via multisig as in `Upgrading multisig` but using `upgrade` instead of `upgrade-multisig` proposal. You will also need to specify `--program <ProgramID>` of the target program in the proposal.

## Delegating upgrade authority

To set upgrade authority of a program to group's protected account you should run `solana program set-upgrade-authority -k keys/alpha.json --new-upgrade-authority <GroupProtectedKey> <ProgramID>`. `-k` is only needed if a program was deployed using non-default key.

To get upgrade authority back from the multisig you need to propose it: `npm run start propose -- delegate-upgrade-authority --group <GroupKey> --key ./keys/alpha.json --target <TargetKey> --new <NewAuthorityKey>`.
Approve the proposal to finalize delegation.

## Viewing of groups and proposals

There is an ability to discover what groups account participates in.

`npm run start view-groups -- --key <PathtoKeypair>`

There is an ability to discover what proposals a group has. Note that the group address is not a group's protected address.

`npm run start view-proposals -- --group <GroupAddress>`