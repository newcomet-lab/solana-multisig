import {
  Connection,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from '@solana/web3.js';
import {Token, TOKEN_PROGRAM_ID, AccountLayout} from '@solana/spl-token';

export async function createTokenAccount(
  connection: Connection,
  protectedGroupKey: PublicKey,
  mint: PublicKey,
  seed: string,
): Promise<TransactionInstruction[]> {
  const balanceNeeded = await Token.getMinBalanceRentForExemptAccount(
    connection,
  );
  const resultingTokenAddress = await PublicKey.createWithSeed(
    protectedGroupKey,
    seed,
    TOKEN_PROGRAM_ID,
  );
  console.log('creating token account: ', resultingTokenAddress.toString());
  const creationPropose = SystemProgram.createAccountWithSeed({
    basePubkey: protectedGroupKey,
    // Pays for creation
    fromPubkey: protectedGroupKey,
    newAccountPubkey: resultingTokenAddress,
    lamports: balanceNeeded,
    programId: TOKEN_PROGRAM_ID,
    seed: seed,
    // Doesn't have an associated type in library, so lints are disabled for this line
    /* eslint-disable */
    space: AccountLayout.span,
    /* eslint-enable */
  });
  const initPropose = Token.createInitAccountInstruction(
    TOKEN_PROGRAM_ID,
    mint,
    resultingTokenAddress,
    protectedGroupKey,
  );

  return [creationPropose, initPropose];
}
