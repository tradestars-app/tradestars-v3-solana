import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  createAssociatedTokenAccount,
  createMint,
  getAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

import { TradestarsArena } from "../target/types/tradestars_arena";

describe("tradestars-arena", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.TradestarsArena as Program<TradestarsArena>;
  const authority = provider.wallet as anchor.Wallet;
  const payer = (provider.wallet as any).payer as Keypair;

  let usdcMint: PublicKey;
  let adminUsdc: PublicKey;
  let user: Keypair;
  let userUsdc: PublicKey;
  let intruder: Keypair;
  let wrongMint: PublicKey;
  let wrongMintAdminAta: PublicKey;

  const [platformConfig] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    program.programId,
  );

  const [platformVault] = PublicKey.findProgramAddressSync(
    [Buffer.from("platform_vault")],
    program.programId,
  );

  function getArenaPdas(arenaId: string, wallet: PublicKey, entryNumber: number) {
    const [arena] = PublicKey.findProgramAddressSync(
      [Buffer.from("arena"), Buffer.from(arenaId)],
      program.programId,
    );

    const [arenaVault] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), arena.toBuffer()],
      program.programId,
    );

    const [entry] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("entry"),
        arena.toBuffer(),
        wallet.toBuffer(),
        Buffer.from([entryNumber]),
      ],
      program.programId,
    );

    return { arena, arenaVault, entry };
  }

  async function sleep(ms: number) {
    await new Promise((resolve) => setTimeout(resolve, ms));
  }

  before(async () => {
    usdcMint = await createMint(
      provider.connection,
      payer,
      authority.publicKey,
      null,
      6,
    );

    adminUsdc = await createAssociatedTokenAccount(
      provider.connection,
      payer,
      usdcMint,
      authority.publicKey,
    );

    user = Keypair.generate();
    let airdropSig = await provider.connection.requestAirdrop(
      user.publicKey,
      1_000_000_000,
    );
    await provider.connection.confirmTransaction(airdropSig, "confirmed");

    intruder = Keypair.generate();
    airdropSig = await provider.connection.requestAirdrop(
      intruder.publicKey,
      1_000_000_000,
    );
    await provider.connection.confirmTransaction(airdropSig, "confirmed");

    userUsdc = await createAssociatedTokenAccount(
      provider.connection,
      payer,
      usdcMint,
      user.publicKey,
    );

    wrongMint = await createMint(
      provider.connection,
      payer,
      authority.publicKey,
      null,
      6,
    );

    wrongMintAdminAta = await createAssociatedTokenAccount(
      provider.connection,
      payer,
      wrongMint,
      authority.publicKey,
    );

    await mintTo(
      provider.connection,
      payer,
      usdcMint,
      adminUsdc,
      payer,
      100_000_000,
    );

    await mintTo(
      provider.connection,
      payer,
      usdcMint,
      userUsdc,
      payer,
      20_000_000,
    );

    await program.methods
      .initializePlatform(authority.publicKey, 1_000)
      .accounts({
        platformConfig,
        platformVault,
        usdcMint,
        authority: authority.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
  });

  it("rejects settle batch with mismatched array lengths", async () => {
    const arenaId = "arena_mismatch_1";
    const { arena, arenaVault } = getArenaPdas(arenaId, user.publicKey, 0);

    const now = Math.floor(Date.now() / 1000);

    await program.methods
      .createArena(arenaId, {
        entryFee: new anchor.BN(10_000_000),
        guaranteedPrizePool: new anchor.BN(10_000_000),
        startTime: new anchor.BN(now - 30),
        endTime: new anchor.BN(now + 1),
        maxEntriesPerUser: 1,
      })
      .accounts({
        authority: authority.publicKey,
        platformConfig,
        arena,
        usdcMint,
        arenaVault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    await sleep(2_100);

    try {
      await program.methods
        .settleAndPayBatch([1, 2], [new anchor.BN(10_000_000)])
        .accounts({
          authority: authority.publicKey,
          platformConfig,
          arena,
          arenaVault,
          platformVault,
          sourceOverlayUsdc: adminUsdc,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();
      assert.fail("settleAndPayBatch should fail for mismatched array lengths");
    } catch (error) {
      assert.include(`${error}`, "BatchLengthMismatch");
    }
  });

  it("settles with automatic overlay and pays winners in batch", async () => {
    const arenaId = "arena_overlay_batch_1";
    const { arena, arenaVault, entry } = getArenaPdas(arenaId, user.publicKey, 0);

    const now = Math.floor(Date.now() / 1000);

    await program.methods
      .createArena(arenaId, {
        entryFee: new anchor.BN(10_000_000),
        guaranteedPrizePool: new anchor.BN(25_000_000),
        startTime: new anchor.BN(now - 30),
        endTime: new anchor.BN(now + 1),
        maxEntriesPerUser: 1,
      })
      .accounts({
        authority: authority.publicKey,
        platformConfig,
        arena,
        usdcMint,
        arenaVault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    await program.methods
      .enterArena(0)
      .accounts({
        user: user.publicKey,
        platformConfig,
        arena,
        entry,
        usdcMint,
        userUsdc,
        arenaVault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    await sleep(2_800);

    const userBefore = Number((await getAccount(provider.connection, userUsdc)).amount);
    const adminBefore = Number((await getAccount(provider.connection, adminUsdc)).amount);

    await program.methods
      .settleAndPayBatch([1], [new anchor.BN(25_000_000)])
      .accounts({
        authority: authority.publicKey,
        platformConfig,
        arena,
        arenaVault,
        platformVault,
        sourceOverlayUsdc: adminUsdc,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts([
        { pubkey: entry, isWritable: true, isSigner: false },
        { pubkey: userUsdc, isWritable: true, isSigner: false },
      ])
      .rpc();

    await program.methods
      .settleAndPayBatch([1], [new anchor.BN(25_000_000)])
      .accounts({
        authority: authority.publicKey,
        platformConfig,
        arena,
        arenaVault,
        platformVault,
        sourceOverlayUsdc: adminUsdc,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts([
        { pubkey: entry, isWritable: true, isSigner: false },
        { pubkey: userUsdc, isWritable: true, isSigner: false },
      ])
      .rpc();

    const userAfter = Number((await getAccount(provider.connection, userUsdc)).amount);
    const adminAfter = Number((await getAccount(provider.connection, adminUsdc)).amount);

    const arenaAccount = await program.account.arena.fetch(arena);
    const entryAccount = await program.account.arenaEntry.fetch(entry);
    const platformVaultBalance = await getAccount(provider.connection, platformVault);
    const arenaVaultBalance = await getAccount(provider.connection, arenaVault);

    assert.equal(userAfter - userBefore, 25_000_000);
    assert.equal(adminBefore - adminAfter, 16_000_000);

    assert.equal(arenaAccount.feeAmount.toNumber(), 1_000_000);
    assert.equal(arenaAccount.overlayFunded.toNumber(), 16_000_000);
    assert.equal(arenaAccount.totalPayoutsSet.toNumber(), 25_000_000);
    assert.equal(arenaAccount.totalRefundsPaid.toNumber(), 0);
    assert.equal("settled" in (arenaAccount.status as any), true);

    assert.equal(entryAccount.payoutAmount.toNumber(), 25_000_000);
    assert.equal(entryAccount.settled, true);

    assert.equal(Number(platformVaultBalance.amount), 1_000_000);
    assert.equal(Number(arenaVaultBalance.amount), 0);
  });

  it("rejects withdraw_fees destination mint mismatch", async () => {
    try {
      await program.methods
        .withdrawFees(null)
        .accounts({
          platformConfig,
          platformVault,
          destination: wrongMintAdminAta,
          authority: authority.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();
      assert.fail("withdrawFees should fail with wrong destination mint");
    } catch (error) {
      assert.include(`${error}`, "InvalidTokenAccount");
    }
  });

  it("supports cancellation + admin batch refunds", async () => {
    const arenaId = "arena_cancel_refund_1";
    const { arena, arenaVault, entry } = getArenaPdas(arenaId, user.publicKey, 0);

    const now = Math.floor(Date.now() / 1000);

    await program.methods
      .createArena(arenaId, {
        entryFee: new anchor.BN(10_000_000),
        guaranteedPrizePool: new anchor.BN(10_000_000),
        startTime: new anchor.BN(now),
        endTime: new anchor.BN(now + 3600),
        maxEntriesPerUser: 1,
      })
      .accounts({
        authority: authority.publicKey,
        platformConfig,
        arena,
        usdcMint,
        arenaVault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    await program.methods
      .enterArena(0)
      .accounts({
        user: user.publicKey,
        platformConfig,
        arena,
        entry,
        usdcMint,
        userUsdc,
        arenaVault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    await program.methods
      .cancelArena("weather")
      .accounts({
        authority: authority.publicKey,
        platformConfig,
        arena,
      })
      .rpc();

    try {
      await program.methods
        .refundBatch()
        .accounts({
          authority: intruder.publicKey,
          platformConfig,
          arena,
          arenaVault,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .remainingAccounts([
          { pubkey: entry, isWritable: true, isSigner: false },
          { pubkey: userUsdc, isWritable: true, isSigner: false },
        ])
        .signers([intruder])
        .rpc();
      assert.fail("refundBatch should fail for unauthorized signer");
    } catch (error) {
      assert.include(`${error}`, "has one constraint was violated");
    }

    const userBefore = Number((await getAccount(provider.connection, userUsdc)).amount);

    await program.methods
      .refundBatch()
      .accounts({
        authority: authority.publicKey,
        platformConfig,
        arena,
        arenaVault,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts([
        { pubkey: entry, isWritable: true, isSigner: false },
        { pubkey: userUsdc, isWritable: true, isSigner: false },
      ])
      .rpc();

    const userAfter = Number((await getAccount(provider.connection, userUsdc)).amount);
    assert.equal(userAfter - userBefore, 10_000_000);

    const arenaAccount = await program.account.arena.fetch(arena);
    const entryAccount = await program.account.arenaEntry.fetch(entry);
    const arenaVaultBalance = await getAccount(provider.connection, arenaVault);

    assert.equal("cancelled" in (arenaAccount.status as any), true);
    assert.equal(arenaAccount.totalRefundsPaid.toNumber(), 10_000_000);
    assert.equal(entryAccount.settled, true);
    assert.equal(entryAccount.payoutAmount.toNumber(), 0);
    assert.equal(Number(arenaVaultBalance.amount), 0);
  });
});
