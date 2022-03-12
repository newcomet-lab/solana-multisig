import {Account, Connection, PublicKey} from '@solana/web3.js';

import {sleep} from './sleep';

export async function newAccountWithLamports(
  connection: Connection,
  lamports = 1000000,
): Promise<Account> {
  const account = new Account();
  await requestAndWaitForAirdrop(connection, account.publicKey, lamports);
  return account;
}

export async function requestAndWaitForAirdrop(
  connection: Connection,
  publicKey: PublicKey,
  lamports = 1000000,
): Promise<void> {
  let retries = 10;
  await connection.requestAirdrop(publicKey, lamports);
  for (;;) {
    await sleep(500);
    if (lamports == (await connection.getBalance(publicKey))) {
      return;
    }
    if (--retries <= 0) {
      break;
    }
    console.log(`Airdrop retry ${retries}`);
  }
  throw new Error(`Airdrop of ${lamports} failed`);
}
