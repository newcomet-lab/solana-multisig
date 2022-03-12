/* eslint-disable @typescript-eslint/no-unsafe-assignment */
/* eslint-disable @typescript-eslint/no-unsafe-member-access */
/* eslint-disable @typescript-eslint/no-unsafe-call */
/* eslint-disable @typescript-eslint/ban-ts-comment */

import {
  Account,
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  Keypair,
} from '@solana/web3.js';
import fs from 'mz/fs';
import {Client as WsClient} from 'rpc-websockets';
// @ts-ignore
import command_line_parser from 'command-line-parser';
import {PropositionKind, Proposition} from './library';
import * as library from './library';

import util from 'util';
import {
  GroupMember,
  GroupData,
  ProtectedAccountConfig,
  InitInstruction,
} from './schema';
import {MultiSig} from './multisig';

import {url, urlTls} from './util/url';
import {Store} from './util/store';
import {Loader} from './loader';

const pathToProgram = 'dist/program/solana_multisig.so';

interface Context {
  connection: Connection;
  multisig: MultiSig;
  commandArgs: any;
}

async function loadProgramId(connection: Connection): Promise<PublicKey> {
  const store = new Store();
  const config = await store.load('config.json');
  const programId = new PublicKey(config.programId);
  await connection.getAccountInfo(programId);
  console.log('Program account: ', programId.toBase58());
  return programId;
}

/**
 * Load the BPF program if not already loaded
 */
async function deployProgramCommon(
  connection: Connection,
  commandArgs: any,
  isAsync: boolean,
): Promise<PublicKey> {
  if (commandArgs.authority == null) {
    throw 'missing authority';
  }
  const authorityAccount = new Account(
    JSON.parse(await fs.readFile(commandArgs.authority, 'utf8')),
  );
  console.log(
    'Deploying program with authority:',
    authorityAccount.publicKey.toBase58(),
  );

  const store = new Store();
  const data = await fs.readFile(pathToProgram);

  // Fund a new payer via airdrop
  /*const payerAccount = await newAccountWithLamports(connection, 10000000000);

  const lamports = await connection.getBalance(payerAccount.publicKey);
  console.log(
    'Using account',
    payerAccount.publicKey.toBase58(),
    'containing',
    lamports / LAMPORTS_PER_SOL,
    'Sol to pay for fees',
  );*/

  // Load the program
  const programAccount = new Account();
  if (isAsync) {
    await Loader.deployAsync(
      connection,
      authorityAccount,
      programAccount,
      authorityAccount,
      data,
    );
  } else {
    await Loader.deploy(
      connection,
      authorityAccount,
      programAccount,
      authorityAccount,
      data,
    );
  }
  const programId = programAccount.publicKey;
  console.log('Program deployed to account', programId.toBase58());

  // Save this info for next time
  await store.save('config.json', {
    url: urlTls,
    programId: programId.toBase58(),
  });

  return programId;
}

/**
 * Load the BPF program if not already loaded
 */
async function deployProgram(
  connection: Connection,
  commandArgs: any,
): Promise<PublicKey> {
  return await deployProgramCommon(connection, commandArgs, false);
}

/**
 * Load the BPF program if not already loaded
 */
async function deployProgramAsync(
  connection: Connection,
  commandArgs: any,
): Promise<PublicKey> {
  return await deployProgramCommon(connection, commandArgs, true);
}

async function init(context: Context): Promise<void> {
  const {commandArgs, connection, multisig} = context;

  if (commandArgs._args.length < 1) {
    throw 'missing members list';
  }
  const membersArg = commandArgs._args.shift();
  if (commandArgs.threshold == null) {
    throw 'missing threshold';
  }
  const threshold = parseInt(commandArgs.threshold);
  const members = membersArg.split(',').map(function (item: any) {
    const [public_key, weight] = item.split(':');
    const pubkey = new PublicKey(public_key);
    return new GroupMember(pubkey, parseInt(weight));
  });

  console.log('sending init');

  const groupData = new GroupData(members, threshold);

  if (commandArgs.payer == null) {
    throw 'missing payer';
  }
  const signerAccount = new Account(
    JSON.parse(await fs.readFile(commandArgs.payer, 'utf8')),
  );
  console.log('signing with account', signerAccount.publicKey.toBase58());

  const groupKey = await multisig.groupAccountKey(groupData);
  console.log('group account:', groupKey.toBase58());

  const protectedAccount = await multisig.protectedAccountKey(groupKey);
  console.log('protected account:', protectedAccount.toBase58());

  const groupAccountInfo = await connection.getAccountInfo(groupKey);
  if (groupAccountInfo !== null) {
    console.log('group account already exists');
    const groupData = multisig.readGroupAccountData(groupAccountInfo);
    console.log('threshold:', groupData.threshold);
    for (const member of groupData.members) {
      console.log(
        'user:',
        new PublicKey(member.publicKey).toBase58(),
        'weight:',
        member.weight,
      );
    }
    return;
  }

  const lamports = await connection.getMinimumBalanceForRentExemption(
    multisig.groupAccountSpace(groupData),
  );
  console.log(lamports, 'lamports will be transfered to the new group account');
  let protectedAccountConfig = null;
  if (commandArgs.createProtected === true) {
    let protectedSpace;
    if (commandArgs.protectedSpace === undefined) {
      protectedSpace = 0;
    } else {
      protectedSpace = parseInt(commandArgs.protectedSpace);
    }

    let protectedLamports;
    const protectedLamportsArg = parseInt(commandArgs.protectedLamports);
    if (!isNaN(protectedLamportsArg)) {
      protectedLamports = protectedLamportsArg;
    } else {
      protectedLamports = await connection.getMinimumBalanceForRentExemption(
        protectedSpace,
      );
    }

    let protectedOwner;
    if (commandArgs.protectedOwner === undefined) {
      protectedOwner = SystemProgram.programId;
    } else {
      protectedOwner = new PublicKey(commandArgs.protectedOwner);
    }

    console.log(
      'protected account will be created with',
      protectedSpace,
      'space and',
      protectedOwner.toBase58(),
      'owner',
    );
    console.log(
      protectedLamports,
      'lamports will be transfered to the protected account',
    );
    protectedAccountConfig = new ProtectedAccountConfig(
      protectedLamports,
      protectedSpace,
      protectedOwner,
    );
  }

  const initInstruction = new InitInstruction(
    groupData,
    lamports,
    protectedAccountConfig,
  );

  const transaction = new Transaction().add(
    await multisig.init(initInstruction, signerAccount.publicKey),
  );

  await sendAndConfirmTransaction(connection, transaction, [signerAccount], {
    commitment: 'singleGossip',
    preflightCommitment: 'singleGossip',
  });
}

async function propose(context: Context): Promise<void> {
  const {commandArgs, connection, multisig} = context;
  if (commandArgs.group == null) {
    throw 'missing group';
  }
  const groupAccount = new PublicKey(commandArgs.group);

  if (commandArgs.key == null) {
    throw 'missing key';
  }
  const signerAccount = new Account(
    JSON.parse(await fs.readFile(commandArgs.key, 'utf8')),
  );
  console.log('signing with account', signerAccount.publicKey.toBase58());

  const protectedAccount = await multisig.protectedAccountKey(groupAccount);
  console.log('protected account:', protectedAccount.toBase58());

  if (commandArgs._args.length < 1) {
    throw 'missing proposed action';
  }
  const proposedAction = commandArgs._args.shift();
  let proposition: Proposition;
  switch (proposedAction) {
    case 'create':
      if (commandArgs.lamports == null) {
        throw 'missing lamports';
      }
      if (commandArgs.finalApprover == null) {
        throw 'must specify the key that will be used to approve last';
      }
      proposition = {
        kind: PropositionKind.Create,
        lamports: parseInt(commandArgs.lamports),
        finalApproverKey: new PublicKey(commandArgs.finalApprover)
      };
      break;
    case 'transfer': {
      if (commandArgs.destination == null) {
        throw 'missing destination';
      }
      if (commandArgs.lamports == null) {
        throw 'missing lamports';
      }
      proposition = {
        kind: PropositionKind.Transfer,
        destination: new PublicKey(commandArgs.destination),
        amount: parseInt(commandArgs.lamports),
      };
      break;
    }
    case 'upgrade': {
      if (commandArgs.buffer == null) {
        throw 'missing buffer';
      }
      if (commandArgs.program == null) {
        throw 'missing program';
      }
      const programUpgrade = new PublicKey(commandArgs.program);
      const bufferUpgrade = new PublicKey(commandArgs.buffer);
      proposition = {
        kind: PropositionKind.Upgrade,
        buffer: bufferUpgrade,
        program: programUpgrade,
      };
      break;
    }
    case 'upgrade-multisig': {
      if (commandArgs.buffer == null) {
        throw 'missing buffer';
      }
      const bufferUpgradeMultisig = new PublicKey(commandArgs.buffer);
      proposition = {
        kind: PropositionKind.UpgradeMultisig,
        buffer: bufferUpgradeMultisig,
      };
      break;
    }
    case 'delegate-upgrade-authority': {
      if (commandArgs.target == null) {
        throw 'missing target to set authority of';
      }
      if (commandArgs.new == null) {
        throw 'missing new authority';
      }
      const targetDelegateUpgrade = new PublicKey(commandArgs.target);
      const newDelegateUpgrade = new PublicKey(commandArgs.new);
      proposition = {
        kind: PropositionKind.DelegateUpgradeAuthority,
        target: targetDelegateUpgrade,
        newAuthority: newDelegateUpgrade,
      };
      break;
    }
    case 'mint-to': {
      if (commandArgs.mint == null) {
        throw 'missing mint pubkey';
      }
      if (commandArgs.destination == null) {
        throw 'missing destination';
      }
      if (commandArgs.amount == null) {
        throw 'missing amount';
      }
      const mintMintoTo = new PublicKey(commandArgs.mint);
      const destMintTo = new PublicKey(commandArgs.destination);
      const amountMintTo = parseInt(commandArgs.amount);
      proposition = {
        kind: PropositionKind.MintTo,
        mint: mintMintoTo,
        destination: destMintTo,
        amount: amountMintTo,
      };
      break;
    }
    // Delegates a mint authority from the protected account to an other account.
    case 'delegate-mint-authority': {
      if (commandArgs.target == null) {
        throw 'missing target to set authority of';
      }
      if (commandArgs.new == null) {
        throw 'missing new authority';
      }
      const targetDelegateMint = new PublicKey(commandArgs.target);
      const newDelegateMint = new PublicKey(commandArgs.new);
      proposition = {
        kind: PropositionKind.DelegateMintAuthority,
        target: targetDelegateMint,
        newAuthority: newDelegateMint,
      };
      break;
    }
    // Delegates a token account authority from the protected account to an other account.
    case 'delegate-token-account-authority': {
      if (commandArgs.target == null) {
        throw 'missing target to set authority of';
      }
      if (commandArgs.new == null) {
        throw 'missing new authority';
      }
      const targetDelegateAccount = new PublicKey(commandArgs.target);
      const newDelegateAccount = new PublicKey(commandArgs.new);
      proposition = {
        kind: PropositionKind.DelegateTokenAuthority,
        target: targetDelegateAccount,
        newAuthority: newDelegateAccount,
      };
      break;
    }
    case 'create-token-account': {
      if (commandArgs.mint == null) {
        throw 'missing mint pubkey';
      }
      if (commandArgs.seed == null) {
        throw 'missing seed for new account';
      }
      const mintCreateToken = new PublicKey(commandArgs.mint);
      proposition = {
        kind: PropositionKind.CreateTokenAccount,
        mint: mintCreateToken,
        seed: commandArgs.seed,
      };
      break;
    }
    case 'transfer-token': {
      if (commandArgs.amount == null) {
        throw 'missing amount';
      }
      if (commandArgs.source == null) {
        throw 'missing source';
      }
      if (commandArgs.destination == null) {
        throw 'missing destination';
      }
      const sourceTransferToken = new PublicKey(commandArgs.source);
      const destinationTransferToken = new PublicKey(commandArgs.destination);
      const amountTransferToken = parseInt(commandArgs.amount);
      proposition = {
        kind: PropositionKind.TransferToken,
        source: sourceTransferToken,
        destination: destinationTransferToken,
        amount: amountTransferToken,
      };
      break;
    }
    default:
      throw 'unknown proposed action';
  }

  const proposalKey = await library.propose(
    connection,
    multisig,
    groupAccount,
    protectedAccount,
    signerAccount,
    proposition,
  );
  console.log(
    'created a proposal account with public key:',
    proposalKey.toBase58(),
  );
}

async function approve(context: Context): Promise<void> {
  const {commandArgs, connection, multisig} = context;
  if (commandArgs.proposal == null) {
    throw 'missing proposal';
  }
  const proposalAccount = new PublicKey(commandArgs.proposal);
  if (commandArgs.key == null) {
    throw 'missing key';
  }
  const signerAccount = new Account(
    JSON.parse(await fs.readFile(commandArgs.key, 'utf8')),
  );
  await library.approve(connection, multisig, signerAccount, proposalAccount);
}

async function closeProposal(context: Context): Promise<void> {
  const {commandArgs, connection, multisig} = context;
  if (commandArgs.proposal == null) {
    throw 'missing proposal';
  }
  if (commandArgs.destination == null) {
    throw 'missing destination to transfer lamports to';
  }
  const proposalAccount = new PublicKey(commandArgs.proposal);
  const destinationKey = new PublicKey(commandArgs.destination);
  if (commandArgs.key == null) {
    throw 'missing key';
  }
  const signerAccount = new Account(
    JSON.parse(await fs.readFile(commandArgs.key, 'utf8')),
  );
  await library.closeProposal(
    connection,
    multisig,
    signerAccount,
    proposalAccount,
    destinationKey,
  );
}

async function viewProposal(context: Context): Promise<void> {
  const {commandArgs, connection, multisig} = context;
  if (commandArgs.proposal == null) {
    throw 'missing proposal';
  }
  const proposalAccount = new PublicKey(commandArgs.proposal);

  const proposalAccountInfo = await connection.getAccountInfo(proposalAccount);
  if (proposalAccountInfo === null) {
    throw 'error: cannot find the proposal account';
  }
  const proposalData = multisig.readProposalAccountData(proposalAccountInfo);

  const groupAccount = new PublicKey(proposalData.config.group);
  const protectedAccount = await multisig.protectedAccountKey(groupAccount);
  console.log('protected account:', protectedAccount.toBase58());
  console.log('current weight:', proposalData.state.current_weight);
  console.log('');
  console.log('proposed instructions:');
  console.group();
  for (const instruction of proposalData.config.instructions) {
    console.log('program: ', new PublicKey(instruction.program_id).toBase58());
    console.log('accounts:');
    console.group();
    for (const account of instruction.accounts) {
      console.log(
        new PublicKey(account.pubkey).toBase58(),
        account.is_signer ? '[signer]' : '',
        account.is_writable ? '[writable]' : '',
      );
    }
    console.groupEnd();
    console.log('data:', instruction.data);
    console.log('');
  }
  console.groupEnd();

  const groupAccountInfo = await connection.getAccountInfo(groupAccount);
  if (groupAccountInfo === null) {
    console.log('group account does not exist!');
  } else {
    console.log('group account:', groupAccount.toBase58());
    const groupData = multisig.readGroupAccountData(groupAccountInfo);
    console.log('  threshold:', groupData.threshold);
    let mask = 1;
    for (const member of groupData.members) {
      const isApproved = (proposalData.state.members.toNumber() & mask) != 0;
      console.log(
        '  user:',
        new PublicKey(member.publicKey).toBase58(),
        'weight:',
        member.weight,
        isApproved ? '[approved]' : '',
      );
      mask = mask << 1;
    }
    return;
  }
}

async function listen(context: Context): Promise<void> {
  const {commandArgs, connection, multisig} = context;
  void multisig;
  if (commandArgs.group == null) {
    throw 'missing group';
  }
  const groupAccount = new PublicKey(commandArgs.group);
  const client = new WsClient('ws://localhost:8900/');
  client.on('open', () => {
    console.log('connected to websocket');
    client
      .call('logsSubscribe', [
        {mentions: [groupAccount.toBase58()]},
        {commitment: 'finalized'},
      ])
      .then(_data => {
        console.log('subscribed');
      })
      .catch(() => 'obligatory catch');

    client.on('logsNotification', function (arg: any) {
      console.log(
        'got notification',
        util.inspect(arg, {showHidden: false, depth: null}),
      );
      const signature = arg.result.value.signature;
      const slot = arg.result.context.slot;

      void (async function () {
        const info = await connection.getConfirmedTransaction(signature);
        console.log(
          'transaction info',
          util.inspect(info, {showHidden: false, depth: null}),
          'slot',
          slot,
        );
      })();
    });
  });

  await new Promise(_resolve => null);
}

async function viewGroups(context: Context) {
  const {commandArgs, connection, multisig} = context;
  const keyPair = Keypair.fromSecretKey(
    Buffer.from(JSON.parse(await fs.readFile(commandArgs.key, 'utf-8'))),
  );
  const groups = await library.getGroups(connection, multisig, keyPair);
  console.log('participating in following groups:');
  console.group();
  for (const {pubkey} of groups) {
    console.log('pubkey:', pubkey.toBase58());
    console.log(
      'protected pubkey:',
      (await multisig.protectedAccountKey(pubkey)).toBase58(),
    );
  }
  console.groupEnd();
}

async function viewProposals(context: Context) {
  const {commandArgs, connection, multisig} = context;
  if (commandArgs.group == null) {
    throw 'specify group';
  }
  const groupAccount = new PublicKey(commandArgs.group);
  const proposal = await library.getProposals(
    connection,
    multisig,
    groupAccount,
  );
  console.log('this group has following open proposals:');
  console.group();
  for (const {pubkey} of proposal) {
    console.log('pubkey:', pubkey.toBase58());
  }
  console.groupEnd();
}

async function main() {
  // Establish connection to the cluster
  const connection = new Connection(url, 'singleGossip');
  const version = await connection.getVersion();
  console.log('Connection to cluster established:', url, version);

  const commandArgs = command_line_parser();
  if (commandArgs._args.length < 1) {
    throw 'missing action';
  }

  const action = commandArgs._args.shift();

  if (action == 'deploy') {
    await deployProgram(connection, commandArgs);
    return;
  }
  if (action == 'deploy-async') {
    await deployProgramAsync(connection, commandArgs);
    return;
  }

  const programId = await loadProgramId(connection);
  const multisig = new MultiSig(programId);
  const context = {connection, multisig, commandArgs};

  if (action == 'init') {
    await init(context);
  } else if (action == 'propose') {
    await propose(context);
  } else if (action == 'approve') {
    await approve(context);
  } else if (action == 'view-proposal') {
    await viewProposal(context);
  } else if (action == 'listen') {
    await listen(context);
  } else if (action == 'view-groups') {
    await viewGroups(context);
  } else if (action == 'view-proposals') {
    await viewProposals(context);
  } else if (action == 'close-proposal') {
    await closeProposal(context);
  } else if (action == 'tmp') {
    const info = await connection.getConfirmedBlock(5595);
    console.log(
      'slot info',
      util.inspect(info, {showHidden: false, depth: null}),
    );
    for (const t of info.transactions) {
      if (t.transaction.signature != null) {
        console.log('signature', t.transaction.signature.toString());
      }
      for (const i of t.transaction.instructions) {
        console.log('program id', i.programId.toBase58());
        if (i.programId.equals(programId)) {
          console.log('found!');
        }
      }
    }
  } else {
    throw 'unknown action';
  }

  console.log('Success');
}

main().then(
  () => process.exit(),
  err => {
    console.error(err);
    process.exit(-1);
  },
);
