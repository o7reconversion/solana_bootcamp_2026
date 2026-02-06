import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import BN from "bn.js";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountIdempotentInstruction,
  createMint,
  getAccount,
  getAssociatedTokenAddressSync,
  mintTo,
} from "@solana/spl-token";
import { assert } from "chai";

describe("anchor_escrow", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const connection = provider.connection;
  const workspace = anchor.workspace as any;
  const program =
    (workspace.anchorEscrow as Program) ||
    (workspace.blueshiftAnchorEscrow as Program);
  if (!program) {
    throw new Error("Program not found in anchor.workspace");
  }

  const wallet = provider.wallet as anchor.Wallet;
  const payer = (provider.wallet as any).payer as Keypair;
  const maker = wallet.publicKey;
  const taker = Keypair.generate();

  let mintA: PublicKey;
  let mintB: PublicKey;
  let makerAtaA: PublicKey;
  let makerAtaB: PublicKey;
  let takerAtaA: PublicKey;
  let takerAtaB: PublicKey;

  const decimals = 0;

  async function airdropIfNeeded(
    pubkey: PublicKey,
    minLamports: number
  ): Promise<void> {
    const balance = await connection.getBalance(pubkey, "confirmed");
    if (balance >= minLamports) {
      return;
    }
    const sig = await connection.requestAirdrop(
      pubkey,
      minLamports - balance
    );
    await connection.confirmTransaction(sig, "confirmed");
  }

  async function ensureAta(
    mint: PublicKey,
    owner: PublicKey
  ): Promise<PublicKey> {
    const ata = getAssociatedTokenAddressSync(
      mint,
      owner,
      true,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );
    const ix = createAssociatedTokenAccountIdempotentInstruction(
      payer.publicKey,
      ata,
      owner,
      mint,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );
    await sendTxWithRetry([ix], [payer]);
    return ata;
  }

  async function sendTxWithRetry(
    ixs: TransactionInstruction[],
    signers: Keypair[],
    attempts = 5
  ): Promise<string> {
    let lastErr: unknown;
    for (let i = 0; i < attempts; i += 1) {
      try {
        const latest = await connection.getLatestBlockhash("confirmed");
        const tx = new Transaction().add(...ixs);
        tx.feePayer = payer.publicKey;
        tx.recentBlockhash = latest.blockhash;
        tx.sign(...signers);
        const sig = await connection.sendRawTransaction(tx.serialize(), {
          skipPreflight: false,
          preflightCommitment: "confirmed",
        });
        await connection.confirmTransaction(
          {
            signature: sig,
            blockhash: latest.blockhash,
            lastValidBlockHeight: latest.lastValidBlockHeight,
          },
          "confirmed"
        );
        return sig;
      } catch (err) {
        lastErr = err;
        const msg = err instanceof Error ? err.message : String(err);
        if (!msg.toLowerCase().includes("blockhash not found")) {
          throw err;
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
    }
    throw lastErr;
  }

  async function isAccountClosed(pubkey: PublicKey): Promise<boolean> {
    const info = await connection.getAccountInfo(pubkey, "confirmed");
    if (!info) {
      return true;
    }
    if (info.lamports === 0) {
      return true;
    }
    if (info.data.length === 0) {
      return true;
    }
    if (info.owner.equals(SystemProgram.programId)) {
      return true;
    }
    return false;
  }

  before(async () => {
    await airdropIfNeeded(payer.publicKey, 2 * LAMPORTS_PER_SOL);
    await airdropIfNeeded(taker.publicKey, 2 * LAMPORTS_PER_SOL);

    mintA = await createMint(
      connection,
      payer,
      maker,
      null,
      decimals,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID
    );
    mintB = await createMint(
      connection,
      payer,
      maker,
      null,
      decimals,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID
    );

    makerAtaA = await ensureAta(mintA, maker);
    makerAtaB = await ensureAta(mintB, maker);
    takerAtaA = await ensureAta(mintA, taker.publicKey);
    takerAtaB = await ensureAta(mintB, taker.publicKey);
  });

  it("make + refund", async () => {
    const seed = new BN(1);
    const depositAmount = new BN(10);
    const receiveAmount = new BN(5);

    await mintTo(
      connection,
      payer,
      mintA,
      makerAtaA,
      payer,
      BigInt(depositAmount.toString()),
      [],
      undefined,
      TOKEN_PROGRAM_ID
    );

    const [escrowPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("escrow"),
        maker.toBuffer(),
        seed.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );

    const vault = getAssociatedTokenAddressSync(
      mintA,
      escrowPda,
      true,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

    const makerBefore = await getAccount(
      connection,
      makerAtaA,
      "confirmed",
      TOKEN_PROGRAM_ID
    );

    const makeSig = await program.methods
      .make(seed, receiveAmount, depositAmount)
      .accounts({
        maker,
        escrow: escrowPda,
        mintA,
        mintB,
        makerAtaA,
        vault,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    await connection.confirmTransaction(makeSig, "confirmed");

    const refundSig = await program.methods
      .refund()
      .accounts({
        maker,
        escrow: escrowPda,
        mintA,
        vault,
        makerAtaA,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    await connection.confirmTransaction(refundSig, "confirmed");

    const makerAfter = await getAccount(
      connection,
      makerAtaA,
      "confirmed",
      TOKEN_PROGRAM_ID
    );
    assert.equal(
      Number(makerAfter.amount),
      Number(makerBefore.amount),
      "maker balance should be restored after refund"
    );

    assert.isTrue(await isAccountClosed(vault), "vault should be closed after refund");
  });

  it("make + take", async () => {
    const seed = new BN(2);
    const depositAmount = new BN(7);
    const receiveAmount = new BN(3);

    await mintTo(
      connection,
      payer,
      mintA,
      makerAtaA,
      payer,
      BigInt(depositAmount.toString()),
      [],
      undefined,
      TOKEN_PROGRAM_ID
    );
    const takerMintAmount = depositAmount.add(receiveAmount);
    await mintTo(
      connection,
      payer,
      mintB,
      takerAtaB,
      payer,
      BigInt(takerMintAmount.toString()),
      [],
      undefined,
      TOKEN_PROGRAM_ID
    );

    const [escrowPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("escrow"),
        maker.toBuffer(),
        seed.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );

    const vault = getAssociatedTokenAddressSync(
      mintA,
      escrowPda,
      true,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

    const makerBBefore = await getAccount(
      connection,
      makerAtaB,
      "confirmed",
      TOKEN_PROGRAM_ID
    );
    const takerABefore = await getAccount(
      connection,
      takerAtaA,
      "confirmed",
      TOKEN_PROGRAM_ID
    );

    const makeSig = await program.methods
      .make(seed, receiveAmount, depositAmount)
      .accounts({
        maker,
        escrow: escrowPda,
        mintA,
        mintB,
        makerAtaA,
        vault,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    await connection.confirmTransaction(makeSig, "confirmed");

    const escrowData: any = await program.account.escrow.fetch(escrowPda);
    const expectedMakerB = Number(escrowData.receive.toString());
    const vaultBefore = await getAccount(
      connection,
      vault,
      "confirmed",
      TOKEN_PROGRAM_ID
    );
    const expectedTakerA = Number(vaultBefore.amount);

    const takeSig = await program.methods
      .take()
      .accounts({
        taker: taker.publicKey,
        maker,
        escrow: escrowPda,
        mintA,
        mintB,
        vault,
        takerAtaA,
        takerAtaB,
        makerAtaB,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([taker])
      .rpc();
    await connection.confirmTransaction(takeSig, "confirmed");

    const makerBAfter = await getAccount(
      connection,
      makerAtaB,
      "confirmed",
      TOKEN_PROGRAM_ID
    );
    const takerAAfter = await getAccount(
      connection,
      takerAtaA,
      "confirmed",
      TOKEN_PROGRAM_ID
    );

    assert.equal(
      Number(makerBAfter.amount) - Number(makerBBefore.amount),
      expectedMakerB,
      "maker should receive Token B"
    );
    assert.equal(
      Number(takerAAfter.amount) - Number(takerABefore.amount),
      expectedTakerA,
      "taker should receive Token A"
    );

    assert.isTrue(await isAccountClosed(vault), "vault should be closed after take");
  });
});
