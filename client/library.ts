import {
  Account,
  Connection,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  Transaction,
  sendAndConfirmTransaction,
  Keypair,
} from '@solana/web3.js';
import {Token, TOKEN_PROGRAM_ID} from '@solana/spl-token';
import * as bs58 from 'bs58';

import {createTokenAccount} from './tokens';
import {
  ProposedAccountMeta,
  ProposeInstruction,
  ProposedInstruction,
  ProposalConfig,
  GroupData,
  ProposalData,
} from './schema';

import {
  ACCOUNT_TYPE_TAG,
  MultiSig,
  readVerifiedGroupAccountData,
  readVerifiedProposalAccountData,
} from './multisig';

import {Loader} from './loader';

export enum PropositionKind {
  Create,
  Transfer,
  Upgrade,
  UpgradeMultisig,
  DelegateUpgradeAuthority,
  DelegateMintAuthority,
  DelegateTokenAuthority,
  MintTo,
  CreateTokenAccount,
  TransferToken,
}

export type Proposition =
  | Create
  | Transfer
  | Upgrade
  | UpgradeMultisig
  | DelegateUpgradeAuthority
  | DelegateMintAuthority
  | DelegateTokenAuthority
  | MintTo
  | CreateTokenAccount
  | TransferToken;

export interface Create {
  kind: PropositionKind.Create;
  lamports: number;
  finalApproverKey: PublicKey;
}

export interface Transfer {
  kind: PropositionKind.Transfer;
  destination: PublicKey;
  amount: number;
}

export interface Upgrade {
  kind: PropositionKind.Upgrade;
  buffer: PublicKey;
  program: PublicKey;
}

export interface UpgradeMultisig {
  kind: PropositionKind.UpgradeMultisig;
  buffer: PublicKey;
}

export interface DelegateUpgradeAuthority {
  kind: PropositionKind.DelegateUpgradeAuthority;
  target: PublicKey;
  newAuthority: PublicKey;
}

export interface DelegateMintAuthority {
  kind: PropositionKind.DelegateMintAuthority;
  target: PublicKey;
  newAuthority: PublicKey;
}

export interface DelegateTokenAuthority {
  kind: PropositionKind.DelegateTokenAuthority;
  target: PublicKey;
  newAuthority: PublicKey;
}

export interface MintTo {
  kind: PropositionKind.MintTo;
  mint: PublicKey;
  destination: PublicKey;
  amount: number;
}

export interface CreateTokenAccount {
  kind: PropositionKind.CreateTokenAccount;
  mint: PublicKey;
  seed: string;
}

export interface TransferToken {
  kind: PropositionKind.TransferToken;
  source: PublicKey;
  destination: PublicKey;
  amount: number;
}

export async function propose(
  connection: Connection,
  multisig: MultiSig,
  groupAccount: PublicKey,
  protectedGroupAccount: PublicKey,
  signerAccount: Account,
  proposition: Proposition,
): Promise<PublicKey> {
  let proposedInstructions: TransactionInstruction[];
  switch (proposition.kind) {
    case PropositionKind.Create:
      proposedInstructions = [
        SystemProgram.createAccount({
          fromPubkey: proposition.finalApproverKey,
          newAccountPubkey: protectedGroupAccount,
          lamports: proposition.lamports,
          space: 0,
          programId: SystemProgram.programId,
        }),
      ];
      break;
    case PropositionKind.Transfer:
      proposedInstructions = [
        SystemProgram.transfer({
          fromPubkey: protectedGroupAccount,
          toPubkey: proposition.destination,
          lamports: proposition.amount,
        }),
      ];
      break;
    case PropositionKind.Upgrade:
      proposedInstructions = [
        await Loader.upgradeInstruction(
          proposition.program,
          proposition.buffer,
          protectedGroupAccount,
          protectedGroupAccount,
        ),
      ];
      break;
    case PropositionKind.UpgradeMultisig:
      proposedInstructions = [
        await Loader.upgradeInstruction(
          multisig.programId,
          proposition.buffer,
          protectedGroupAccount,
          protectedGroupAccount,
        ),
      ];
      break;
    case PropositionKind.DelegateUpgradeAuthority:
      proposedInstructions = [
        await Loader.setUpgradeAuthorityInstruction(
          proposition.target,
          protectedGroupAccount,
          proposition.newAuthority,
        ),
      ];
      break;
    case PropositionKind.DelegateTokenAuthority:
      proposedInstructions = [
        Token.createSetAuthorityInstruction(
          TOKEN_PROGRAM_ID,
          proposition.target,
          proposition.newAuthority,
          'AccountOwner',
          protectedGroupAccount,
          [],
        ),
      ];
      break;
    case PropositionKind.DelegateMintAuthority:
      proposedInstructions = [
        Token.createSetAuthorityInstruction(
          TOKEN_PROGRAM_ID,
          proposition.target,
          proposition.newAuthority,
          'MintTokens',
          protectedGroupAccount,
          [],
        ),
      ];
      break;
    case PropositionKind.MintTo:
      proposedInstructions = [
        Token.createMintToInstruction(
          TOKEN_PROGRAM_ID,
          proposition.mint,
          proposition.destination,
          // Authority is implied to be group's protected account
          protectedGroupAccount,
          [],
          proposition.amount,
        ),
      ];
      break;
    case PropositionKind.CreateTokenAccount:
      proposedInstructions = await createTokenAccount(
        connection,
        protectedGroupAccount,
        proposition.mint,
        proposition.seed,
      );
      break;
    case PropositionKind.TransferToken:
      proposedInstructions = [
        Token.createTransferInstruction(
          TOKEN_PROGRAM_ID,
          proposition.source,
          proposition.destination,
          // Authority is implied to be group's protected account
          protectedGroupAccount,
          [],
          proposition.amount,
        ),
      ];
      break;
    default:
      throw 'unsupported proposition';
  }
  return await sendPropose(
    connection,
    multisig,
    signerAccount,
    groupAccount,
    proposedInstructions,
  );
}

export async function approve(
  connection: Connection,
  multisig: MultiSig,
  signer: Account,
  proposal: PublicKey,
): Promise<void> {
  console.log('signing with account', signer.publicKey.toBase58());

  const proposalAccountInfo = await connection.getAccountInfo(proposal);
  if (proposalAccountInfo === null) {
    throw 'error: cannot find the proposal account';
  }

  const proposalData = multisig.readProposalAccountData(proposalAccountInfo);

  const groupAccount = new PublicKey(proposalData.config.group);
  console.log('group account:', groupAccount.toBase58());
  const protectedAccount = await multisig.protectedAccountKey(groupAccount);
  console.log('protected account:', protectedAccount.toBase58());

  const transaction = new Transaction().add(
    await multisig.approve(proposal, proposalData.config, signer.publicKey),
  );
  await sendAndConfirmTransaction(connection, transaction, [signer], {
    commitment: 'singleGossip',
    preflightCommitment: 'singleGossip',
  });
}

export async function closeProposal(
  connection: Connection,
  multisig: MultiSig,
  signer: Account,
  proposal: PublicKey,
  destinationKey: PublicKey,
): Promise<void> {
  console.log('signing with account', signer.publicKey.toBase58());

  const proposalAccountInfo = await connection.getAccountInfo(proposal);
  if (proposalAccountInfo === null) {
    throw 'error: cannot find the proposal account';
  }

  const proposalData = multisig.readProposalAccountData(proposalAccountInfo);

  const groupAccount = new PublicKey(proposalData.config.group);
  console.log('group account:', groupAccount.toBase58());
  const protectedAccount = await multisig.protectedAccountKey(groupAccount);
  console.log('protected account:', protectedAccount.toBase58());

  const transaction = new Transaction().add(
    multisig.closeProposal(proposal, signer.publicKey, destinationKey),
  );

  await sendAndConfirmTransaction(connection, transaction, [signer], {
    commitment: 'singleGossip',
    preflightCommitment: 'singleGossip',
  });
}

export async function sendPropose(
  connection: Connection,
  multisig: MultiSig,
  signerAccount: Account,
  groupAccount: PublicKey,
  instructions: TransactionInstruction[],
): Promise<PublicKey> {
  const transaction = new Transaction();
  const proposedInstructions = instructions.map(
    instruction =>
      new ProposedInstruction(
        instruction.programId,
        instruction.keys.map(
          key =>
            new ProposedAccountMeta(key.pubkey, key.isSigner, key.isWritable),
        ),
        instruction.data,
      ),
  );

  const salt = Date.now();
  const proposalConfig = new ProposalConfig({
    group: Uint8Array.from(groupAccount.toBuffer()),
    instructions: proposedInstructions,
    author: Uint8Array.from(signerAccount.publicKey.toBuffer()),
    salt
  });
  const proposalKey = await multisig.proposalAccountKey(proposalConfig);

  const rent = await connection.getMinimumBalanceForRentExemption(
    multisig.proposalAccountSpace(proposalConfig),
  );

  const proposeInstruction = new ProposeInstruction(
    proposedInstructions,
    rent,
    salt,
  );

  transaction.add(
    await multisig.propose(
      proposeInstruction,
      groupAccount,
      signerAccount.publicKey,
    ),
  );
  await sendAndConfirmTransaction(connection, transaction, [signerAccount], {
    commitment: 'singleGossip',
    preflightCommitment: 'singleGossip',
  });
  return proposalKey;
}

export async function getGroups(
  connection: Connection,
  multisig: MultiSig,
  signerKeyPair: Keypair,
): Promise<{info: GroupData; pubkey: PublicKey}[]> {
  const bytes = bs58.encode([ACCOUNT_TYPE_TAG['group']]);
  const memFilter = {memcmp: {bytes, offset: 0}};
  const accounts = await connection.getProgramAccounts(multisig.programId, {
    commitment: 'confirmed',
    filters: [memFilter],
  });
  const signerKeyBytes = signerKeyPair.publicKey.toBytes();
  const groups = accounts
    .map(({account, pubkey}) => ({
      info: readVerifiedGroupAccountData(account),
      pubkey: pubkey,
    }))
    .filter(({info}) =>
      info.members.some(member =>
        arrayEquals(member.publicKey, signerKeyBytes),
      ),
    );
  return groups;
}

export async function getProposals(
  connection: Connection,
  multisig: MultiSig,
  groupKey: PublicKey,
): Promise<{info: ProposalData; pubkey: PublicKey}[]> {
  const arrayOne = new Uint8Array([ACCOUNT_TYPE_TAG['proposal']]);
  const arrayTwo = groupKey.toBytes();
  const mergedArray = new Uint8Array(arrayOne.length + arrayTwo.length);
  mergedArray.set(arrayOne);
  mergedArray.set(arrayTwo, arrayOne.length);

  const bytes = bs58.encode(mergedArray);
  const memFilter = {memcmp: {bytes, offset: 0}};
  const accounts = await connection.getProgramAccounts(multisig.programId, {
    commitment: 'confirmed',
    filters: [memFilter],
  });
  return accounts.map(({account, pubkey}) => ({
    info: readVerifiedProposalAccountData(account),
    pubkey,
  }));
}

function arrayEquals(a: Uint8Array, b: Uint8Array) {
  return a.length === b.length && a.every((val, index) => val === b[index]);
}
