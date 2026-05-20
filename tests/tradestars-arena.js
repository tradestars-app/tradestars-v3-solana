import * as anchor from "@coral-xyz/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  ExtensionType,
  TOKEN_2022_PROGRAM_ID,
  createInitializeAccountInstruction,
  createInitializeImmutableOwnerInstruction,
  createTransferCheckedInstruction,
  getAccount,
  getAccountLen,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { assert } from "chai";
import { keccak_256 } from "../node_modules/.pnpm/node_modules/@noble/hashes/sha3.js";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";

const { BN } = anchor.default;

describe("tradestars-arena", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.TradestarsArena;
  const authority = provider.wallet;

  const [platformConfig] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    program.programId
  );
  const [tusdcMint] = PublicKey.findProgramAddressSync(
    [Buffer.from("tusdc_mint")],
    program.programId
  );

  const mintingAuthority = Keypair.generate();
  const arenaOperator = Keypair.generate();
  const treasury = Keypair.generate();
  const intruder = Keypair.generate();

  const bn = (value) => new BN(value.toString());
  const zeroPubkey = new PublicKey(new Uint8Array(32));

  const bytes32Buffer = (label) => {
    const buffer = Buffer.alloc(32);
    buffer.write(label);
    return buffer;
  };

  const u32LeBuffer = (value) => {
    const buffer = Buffer.alloc(4);
    buffer.writeUInt32LE(value);
    return buffer;
  };

  const hashPair = (left, right) => {
    const ordered = Buffer.compare(left, right) <= 0 ? [left, right] : [right, left];
    return Buffer.from(keccak_256(Buffer.concat(ordered)));
  };

  const merkleLeaf = (
    arenaId,
    settlementVersion,
    user,
    lockedAmount,
    payoutAmount
  ) => {
    const version = Buffer.alloc(4);
    version.writeUInt32LE(settlementVersion);
    const locked = Buffer.alloc(8);
    locked.writeBigUInt64LE(BigInt(lockedAmount));
    const payout = Buffer.alloc(8);
    payout.writeBigUInt64LE(BigInt(payoutAmount));

    return Buffer.from(
      keccak_256(
        Buffer.concat([arenaId, version, user.toBuffer(), locked, payout])
      )
    );
  };

  const buildMerkle = (leaves) => {
    if (leaves.length === 1) {
      return { root: leaves[0], proofs: [[], []].slice(0, 1) };
    }
    if (leaves.length !== 2) {
      throw new Error("test helper only supports one or two leaves");
    }
    return {
      root: hashPair(leaves[0], leaves[1]),
      proofs: [[leaves[1]], [leaves[0]]],
    };
  };

  const sleep = async (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const confirm = async (signature) =>
    provider.connection.confirmTransaction(signature, "confirmed");

  const airdrop = async (pubkey, lamports = 2_000_000_000) => {
    const signature = await provider.connection.requestAirdrop(pubkey, lamports);
    await provider.connection.confirmTransaction({ signature }, "confirmed");
  };

  const userPda = (user) =>
    PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.toBuffer()],
      program.programId
    )[0];

  const arenaPda = (arenaId) =>
    PublicKey.findProgramAddressSync(
      [Buffer.from("arena"), arenaId],
      program.programId
    )[0];

  const positionPda = (arena, user) =>
    PublicKey.findProgramAddressSync(
      [Buffer.from("position"), arena.toBuffer(), user.toBuffer()],
      program.programId
    )[0];

  const tusdcAta = (owner, allowOwnerOffCurve = false) =>
    getAssociatedTokenAddressSync(
      tusdcMint,
      owner,
      allowOwnerOffCurve,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

  const getAtaAmount = async (owner, allowOwnerOffCurve = false) =>
    Number(
      (
        await getAccount(
          provider.connection,
          tusdcAta(owner, allowOwnerOffCurve),
          "processed",
          TOKEN_2022_PROGRAM_ID
        )
      ).amount
    );

  const createUser = async (depositAmount = 10_000_000) => {
    const user = Keypair.generate();
    await airdrop(user.publicKey);
    await depositCollateral(
      user.publicKey,
      depositAmount,
      bytes32Buffer(`dep-${user.publicKey.toBase58().slice(0, 8)}`)
    );
    return user;
  };

  const createExtraTokenAccount = async (owner) => {
    const tokenAccount = Keypair.generate();
    const accountLen = getAccountLen([
      ExtensionType.ImmutableOwner,
      ExtensionType.NonTransferableAccount,
    ]);
    const lamports = await provider.connection.getMinimumBalanceForRentExemption(
      accountLen
    );
    const tx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: owner.publicKey,
        newAccountPubkey: tokenAccount.publicKey,
        space: accountLen,
        lamports,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeImmutableOwnerInstruction(
        tokenAccount.publicKey,
        TOKEN_2022_PROGRAM_ID
      ),
      createInitializeAccountInstruction(
        tokenAccount.publicKey,
        tusdcMint,
        owner.publicKey,
        TOKEN_2022_PROGRAM_ID
      )
    );
    await provider.sendAndConfirm(tx, [owner, tokenAccount]);
    return tokenAccount.publicKey;
  };

  const expectFailure = async (promise, needle) => {
    try {
      await promise;
      assert.fail("expected transaction to fail");
    } catch (error) {
      if (needle) {
        assert.include(`${error}`, needle);
      }
    }
  };

  const depositCollateral = async (
    user,
    amount,
    baseTxHash,
    logIndex = 0,
    signer = mintingAuthority
  ) => {
    await program.methods
      .depositCollateral(bn(amount), [...baseTxHash], logIndex)
      .accounts({
        platformConfig,
        tusdcMint,
        user,
        userAccount: userPda(user),
        depositMarker: PublicKey.findProgramAddressSync(
          [Buffer.from("deposit"), baseTxHash, u32LeBuffer(logIndex)],
          program.programId
        )[0],
        userTusdc: tusdcAta(user),
        mintingAuthority: signer.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([signer])
      .rpc();
  };

  const createArena = async ({
    arenaId,
    creator = authority.publicKey,
    creatorSigner = authority,
    entryFee,
    feeBps,
    guaranteedPrizeTarget,
    startTime,
    entryCloseTime = startTime,
    endTime,
    metadataHash = bytes32Buffer("metadata"),
  }) => {
    const arena = arenaPda(arenaId);
    const builder = program.methods
      .createArena(arenaId, {
        creator,
        entryFee: bn(entryFee),
        feeBps,
        guaranteedPrizeTarget: bn(guaranteedPrizeTarget),
        startTime: bn(startTime),
        entryCloseTime: bn(entryCloseTime),
        endTime: bn(endTime),
        metadataHash: [...metadataHash],
      })
      .accounts({
        platformConfig,
        arena,
        creator,
        creatorUserAccount: userPda(creator),
        tusdcMint,
        creatorTusdc: tusdcAta(creator),
        authority: arenaOperator.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      });

    if ("payer" in creatorSigner) {
      await builder.signers([arenaOperator]).rpc();
    } else {
      await builder.signers([arenaOperator, creatorSigner]).rpc();
    }

    return arena;
  };

  const joinArena = async ({ arena, user }) => {
    await program.methods
      .joinArena()
      .accounts({
        user: user.publicKey,
        userAccount: userPda(user.publicKey),
        arena,
        position: positionPda(arena, user.publicKey),
        tusdcMint,
        userTusdc: tusdcAta(user.publicKey),
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();
  };

  before(async () => {
    await Promise.all([
      airdrop(mintingAuthority.publicKey),
      airdrop(arenaOperator.publicKey),
      airdrop(treasury.publicKey),
      airdrop(intruder.publicKey),
    ]);

    await program.methods
      .initializePlatform(
        arenaOperator.publicKey,
        treasury.publicKey,
        mintingAuthority.publicKey,
        bn(2),
        bn(3)
      )
      .accounts({
        platformConfig,
        tusdcMint,
        authority: authority.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    await depositCollateral(authority.publicKey, 20_000_000, bytes32Buffer("creator-deposit"));
    await depositCollateral(treasury.publicKey, 20_000_000, bytes32Buffer("treasury-deposit"));
  });

  it("mints soulbound deposits only from the minting authority, rejects unauthorized callers, and blocks peer transfers", async () => {
    const userA = Keypair.generate();
    const userB = Keypair.generate();
    await Promise.all([airdrop(userA.publicKey), airdrop(userB.publicKey)]);

    const depositHash = bytes32Buffer("user-a-deposit");
    await expectFailure(
      depositCollateral(userA.publicKey, 5_000_000, depositHash, 0, intruder),
      "InvalidMintingAuthority"
    );

    await depositCollateral(userA.publicKey, 5_000_000, depositHash);
    await depositCollateral(userA.publicKey, 500_000, depositHash, 1);
    await depositCollateral(
      userB.publicKey,
      1_000_000,
      bytes32Buffer("user-b-deposit")
    );

    await expectFailure(
      depositCollateral(userA.publicKey, 1, depositHash, 0),
      "already in use"
    );

    const userAAccount = await program.account.userAccount.fetch(userPda(userA.publicKey));
    const userBAccount = await program.account.userAccount.fetch(userPda(userB.publicKey));
    assert.equal(userAAccount.totalBalance.toNumber(), 5_500_000);
    assert.equal(userBAccount.totalBalance.toNumber(), 1_000_000);

    const transferIx = createTransferCheckedInstruction(
      tusdcAta(userA.publicKey),
      tusdcMint,
      tusdcAta(userB.publicKey),
      userA.publicKey,
      100_000,
      6,
      [],
      TOKEN_2022_PROGRAM_ID
    );

    await expectFailure(provider.sendAndConfirm(new Transaction().add(transferIx), [userA]));
  });

  it("rejects zero-value role updates", async () => {
    await expectFailure(
      program.methods
        .updatePlatformConfig(
          null,
          zeroPubkey,
          null,
          null,
          null,
          null
        )
        .accounts({
          platformConfig,
          authority: authority.publicKey,
        })
        .rpc(),
      "InvalidRoleKey"
    );

    await expectFailure(
      program.methods
        .updatePlatformConfig(
          null,
          null,
          zeroPubkey,
          null,
          null,
          null
        )
        .accounts({
          platformConfig,
          authority: authority.publicKey,
        })
        .rpc(),
      "InvalidTreasuryWallet"
    );

    await expectFailure(
      program.methods
        .updatePlatformConfig(
          null,
          null,
          null,
          zeroPubkey,
          null,
          null
        )
        .accounts({
          platformConfig,
          authority: authority.publicKey,
        })
        .rpc(),
      "InvalidRoleKey"
    );

    await expectFailure(
      program.methods
        .updatePlatformConfig(
          zeroPubkey,
          null,
          null,
          null,
          null,
          null
        )
        .accounts({
          platformConfig,
          authority: authority.publicKey,
        })
        .rpc(),
      "InvalidRoleKey"
    );
  });

  it("locks creator guarantees at creation and enforces the start-time cutoff for joins and cancellation", async () => {
    const creator = await createUser(2_000_000);
    const player = await createUser(1_000_000);
    const arenaId = bytes32Buffer("start-gated");
    const now = Math.floor(Date.now() / 1000);
    const arena = await createArena({
      arenaId,
      creator: creator.publicKey,
      creatorSigner: creator,
      entryFee: 100_000,
      feeBps: 500,
      guaranteedPrizeTarget: 500_000,
      startTime: now + 2,
      endTime: now + 10,
    });

    const creatorAccount = await program.account.userAccount.fetch(userPda(creator.publicKey));
    assert.equal(creatorAccount.totalBalance.toNumber(), 2_000_000);
    assert.equal(creatorAccount.inPlayDebt.toNumber(), 500_000);
    assert.equal(await getAtaAmount(creator.publicKey), 1_500_000);

    await joinArena({ arena, user: player });
    await sleep(2200);

    await expectFailure(joinArena({ arena, user: player }), "InvalidTime");
    await expectFailure(
      program.methods
        .cancelArena()
        .accounts({
          platformConfig,
          arena,
          authority: arenaOperator.publicKey,
        })
        .signers([arenaOperator])
        .rpc(),
      "InvalidTime"
    );
  });

  it("burns entry commitments, tracks fees on join, and blocks over-withdrawals while debt is live", async () => {
    const user = await createUser(2_000_000);
    const arenaId = bytes32Buffer("join-arena");
    const now = Math.floor(Date.now() / 1000);
    const arena = await createArena({
      arenaId,
      entryFee: 200_000,
      feeBps: 1_000,
      guaranteedPrizeTarget: 0,
      startTime: now + 120,
      endTime: now + 240,
    });

    const beforeAta = await getAtaAmount(user.publicKey);
    for (let i = 0; i < 10; i += 1) {
      await joinArena({ arena, user });
    }

    await expectFailure(joinArena({ arena, user }));
    await expectFailure(
      program.methods
        .withdrawRequest(bn(1), bn(1))
        .accounts({
          user: user.publicKey,
          userAccount: userPda(user.publicKey),
          tusdcMint,
          userTusdc: tusdcAta(user.publicKey),
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        })
        .signers([user])
        .rpc(),
      "InsufficientAvailableBalance"
    );

    const arenaAccount = await program.account.arenaAccount.fetch(arena);
    const position = await program.account.arenaPosition.fetch(positionPda(arena, user.publicKey));
    const userAccount = await program.account.userAccount.fetch(userPda(user.publicKey));
    const afterAta = await getAtaAmount(user.publicKey);

    assert.equal(position.entryCount, 10);
    assert.equal(position.lockedAmount.toNumber(), 2_000_000);
    assert.equal(userAccount.totalBalance.toNumber(), 2_000_000);
    assert.equal(userAccount.inPlayDebt.toNumber(), 2_000_000);
    assert.equal(arenaAccount.totalEntryFeesLocked.toNumber(), 2_000_000);
    assert.equal(arenaAccount.feeAccrued.toNumber(), 200_000);
    assert.equal(arenaAccount.totalPool.toNumber(), 1_800_000);
    assert.equal(afterAta, beforeAta - 2_000_000);
  });

  it("moves arenas into disputed state at the threshold, blocks settlement, and allows permissionless disputed cancellation", async () => {
    const userA = await createUser(1_000_000);
    const userB = await createUser(1_000_000);
    const arenaId = bytes32Buffer("dispute-arena");
    const now = Math.floor(Date.now() / 1000);
    const arena = await createArena({
      arenaId,
      entryFee: 200_000,
      feeBps: 0,
      guaranteedPrizeTarget: 0,
      startTime: now + 2,
      endTime: now + 4,
    });

    await joinArena({ arena, user: userA });
    await joinArena({ arena, user: userB });
    await sleep(4200);

    const leafA = merkleLeaf(arenaId, 1, userA.publicKey, 200_000, 300_000);
    const leafB = merkleLeaf(arenaId, 1, userB.publicKey, 200_000, 100_000);
    const { root: badRoot } = buildMerkle([leafA, leafB]);

    await program.methods
      .postSettlementRoot([...badRoot])
      .accounts({
        platformConfig,
        arena,
        authority: arenaOperator.publicKey,
      })
      .signers([arenaOperator])
      .rpc();

    await program.methods
      .submitDispute()
      .accounts({
        platformConfig,
        arena,
        position: positionPda(arena, userA.publicKey),
        user: userA.publicKey,
      })
      .signers([userA])
      .rpc();

    let arenaAccount = await program.account.arenaAccount.fetch(arena);
    assert.deepEqual(arenaAccount.status, { disputed: {} });
    assert.equal(arenaAccount.disputeCount, 1);

    await expectFailure(
      program.methods
        .claimWinnings(bn(200_000), bn(300_000), [])
        .accounts({
          platformConfig,
          arena,
          userAccount: userPda(userA.publicKey),
          position: positionPda(arena, userA.publicKey),
          tusdcMint,
          userTusdc: tusdcAta(userA.publicKey),
          user: userA.publicKey,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        })
        .signers([userA])
        .rpc(),
      "ArenaNotSettled"
    );

    await program.methods
      .cancelDisputedArena()
      .accounts({
        arena,
        caller: intruder.publicKey,
      })
      .signers([intruder])
      .rpc();

    arenaAccount = await program.account.arenaAccount.fetch(arena);
    assert.deepEqual(arenaAccount.status, { cancelled: {} });
  });

  it("allows authority to pause deposits and anyone to stale-cancel unresolved arenas after timeout", async () => {
    const pausedUser = Keypair.generate();
    await airdrop(pausedUser.publicKey);

    await program.methods
      .setDepositsPaused(true)
      .accounts({
        platformConfig,
        authority: authority.publicKey,
      })
      .rpc();

    await expectFailure(
      program.methods
        .setDepositsPaused(false)
        .accounts({
          platformConfig,
          authority: intruder.publicKey,
        })
        .signers([intruder])
        .rpc(),
      "Unauthorized"
    );

    await expectFailure(
      depositCollateral(pausedUser.publicKey, 1_000_000, bytes32Buffer("paused-deposit")),
      "DepositsPaused"
    );

    await program.methods
      .setDepositsPaused(false)
      .accounts({
        platformConfig,
        authority: authority.publicKey,
      })
      .rpc();

    await depositCollateral(pausedUser.publicKey, 1_000_000, bytes32Buffer("unpaused-deposit"));

    const userA = await createUser(1_000_000);
    const userB = await createUser(1_000_000);
    const arenaId = bytes32Buffer("stale-cancel");
    const now = Math.floor(Date.now() / 1000);
    const arena = await createArena({
      arenaId,
      entryFee: 200_000,
      feeBps: 0,
      guaranteedPrizeTarget: 500_000,
      startTime: now + 2,
      endTime: now + 4,
    });

    await joinArena({ arena, user: userA });
    await joinArena({ arena, user: userB });
    await sleep(4200);

    await expectFailure(
      program.methods
        .cancelStaleArena()
        .accounts({
          platformConfig,
          arena,
          caller: intruder.publicKey,
        })
        .signers([intruder])
        .rpc(),
      "ArenaNotStale"
    );

    await sleep(3200);

    await program.methods
      .cancelStaleArena()
      .accounts({
        platformConfig,
        arena,
        caller: intruder.publicKey,
      })
      .signers([intruder])
      .rpc();

    const arenaAccount = await program.account.arenaAccount.fetch(arena);
    assert.deepEqual(arenaAccount.status, { cancelled: {} });

    const refundBatchSig = await program.methods
      .refundArenaBatch()
      .accounts({
        platformConfig,
        arena,
        authority: arenaOperator.publicKey,
        tusdcMint,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .remainingAccounts([
        { pubkey: userPda(userA.publicKey), isWritable: true, isSigner: false },
        { pubkey: tusdcAta(userA.publicKey), isWritable: true, isSigner: false },
        { pubkey: positionPda(arena, userA.publicKey), isWritable: true, isSigner: false },
      ])
      .signers([arenaOperator])
      .rpc();
    await confirm(refundBatchSig);

    const refundSig = await program.methods
      .claimRefund()
      .accounts({
        platformConfig,
        arena,
        userAccount: userPda(userB.publicKey),
        position: positionPda(arena, userB.publicKey),
        tusdcMint,
        userTusdc: tusdcAta(userB.publicKey),
        user: userB.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .signers([userB])
      .rpc();
    await confirm(refundSig);

    const finalizeSig = await program.methods
      .finalizeArena()
      .accounts({
        platformConfig,
        arena,
        creator: authority.publicKey,
        creatorAccount: userPda(authority.publicKey),
        creatorTusdc: tusdcAta(authority.publicKey),
        treasuryAccount: userPda(treasury.publicKey),
        treasuryWallet: treasury.publicKey,
        tusdcMint,
        treasuryTusdc: tusdcAta(treasury.publicKey),
        payer: intruder.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([intruder])
      .rpc();
    await confirm(finalizeSig);
  });

  it("rejects non-canonical token accounts in admin batch settlement and refund flows", async () => {
    const settlingUser = await createUser(2_000_000);
    const settlementArenaId = bytes32Buffer("batch-ata-settle");
    const now = Math.floor(Date.now() / 1000);
    const settlementArena = await createArena({
      arenaId: settlementArenaId,
      entryFee: 500_000,
      feeBps: 0,
      guaranteedPrizeTarget: 0,
      startTime: now + 2,
      endTime: now + 6,
    });

    await joinArena({ arena: settlementArena, user: settlingUser });
    await sleep(3000);

    const settlementLeaf = merkleLeaf(
      settlementArenaId,
      1,
      settlingUser.publicKey,
      500_000,
      500_000
    );
    const { root: settlementRoot, proofs: settlementProofs } = buildMerkle([settlementLeaf]);
    let rootPosted = false;
    for (let attempt = 0; attempt < 6; attempt += 1) {
      try {
        await program.methods
          .postSettlementRoot([...settlementRoot])
          .accounts({
            platformConfig,
            arena: settlementArena,
            authority: arenaOperator.publicKey,
          })
          .signers([arenaOperator])
          .rpc();
        rootPosted = true;
        break;
      } catch (error) {
        if (!`${error}`.includes("InvalidTime") || attempt === 5) {
          throw error;
        }
        await sleep(1000);
      }
    }
    assert.isTrue(rootPosted);
    await sleep(2500);

    const altSettlementToken = await createExtraTokenAccount(settlingUser);
    await expectFailure(
      program.methods
        .settleArenaBatch([
          {
            lockedAmount: bn(500_000),
            payoutAmount: bn(500_000),
            proof: settlementProofs[0].map((node) => [...node]),
          },
        ])
        .accounts({
          platformConfig,
          arena: settlementArena,
          authority: arenaOperator.publicKey,
          tusdcMint,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .remainingAccounts([
          { pubkey: userPda(settlingUser.publicKey), isWritable: true, isSigner: false },
          { pubkey: altSettlementToken, isWritable: true, isSigner: false },
          {
            pubkey: positionPda(settlementArena, settlingUser.publicKey),
            isWritable: true,
            isSigner: false,
          },
        ])
        .signers([arenaOperator])
        .rpc(),
      "InvalidTokenAccount"
    );

    const refundingUser = await createUser(2_000_000);
    const refundArenaId = bytes32Buffer("batch-ata-refund");
    const refundArena = await createArena({
      arenaId: refundArenaId,
      entryFee: 500_000,
      feeBps: 0,
      guaranteedPrizeTarget: 0,
      startTime: Math.floor(Date.now() / 1000) + 60,
      endTime: Math.floor(Date.now() / 1000) + 120,
    });
    await joinArena({ arena: refundArena, user: refundingUser });
    await program.methods
      .cancelArena()
      .accounts({
        platformConfig,
        arena: refundArena,
        authority: arenaOperator.publicKey,
      })
      .signers([arenaOperator])
      .rpc();

    const altRefundToken = await createExtraTokenAccount(refundingUser);
    await expectFailure(
      program.methods
        .refundArenaBatch()
        .accounts({
          platformConfig,
          arena: refundArena,
          authority: arenaOperator.publicKey,
          tusdcMint,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .remainingAccounts([
          { pubkey: userPda(refundingUser.publicKey), isWritable: true, isSigner: false },
          { pubkey: altRefundToken, isWritable: true, isSigner: false },
          {
            pubkey: positionPda(refundArena, refundingUser.publicKey),
            isWritable: true,
            isSigner: false,
          },
        ])
        .signers([arenaOperator])
        .rpc(),
      "InvalidTokenAccount"
    );
  });

  it("settles in admin batches with user fallback claims, finalizes creator guarantees, and mints fees to treasury", async () => {
    const userA = await createUser(10_000_000);
    const userB = await createUser(10_000_000);
    const arenaId = bytes32Buffer("settle-arena");
    const now = Math.floor(Date.now() / 1000);
    const arena = await createArena({
      arenaId,
      entryFee: 4_000_000,
      feeBps: 1_000,
      guaranteedPrizeTarget: 5_000_000,
      startTime: now + 2,
      endTime: now + 4,
    });

    const creatorAtaBefore = await getAtaAmount(authority.publicKey);
    assert.equal(creatorAtaBefore, 15_000_000);

    await joinArena({ arena, user: userA });
    await joinArena({ arena, user: userB });
    await sleep(4200);

    const leafA = merkleLeaf(arenaId, 1, userA.publicKey, 4_000_000, 4_000_000);
    const leafB = merkleLeaf(arenaId, 1, userB.publicKey, 4_000_000, 3_200_000);
    const { root, proofs } = buildMerkle([leafA, leafB]);

    await program.methods
      .postSettlementRoot([...root])
      .accounts({
        platformConfig,
        arena,
        authority: arenaOperator.publicKey,
      })
      .signers([arenaOperator])
      .rpc();

    await sleep(2500);

    const batchSig = await program.methods
      .settleArenaBatch([
        {
          lockedAmount: bn(4_000_000),
          payoutAmount: bn(4_000_000),
          proof: proofs[0].map((node) => [...node]),
        },
      ])
      .accounts({
        platformConfig,
        arena,
        authority: arenaOperator.publicKey,
        tusdcMint,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .remainingAccounts([
        { pubkey: userPda(userA.publicKey), isWritable: true, isSigner: false },
        { pubkey: tusdcAta(userA.publicKey), isWritable: true, isSigner: false },
        { pubkey: positionPda(arena, userA.publicKey), isWritable: true, isSigner: false },
      ])
      .signers([arenaOperator])
      .rpc();
    await confirm(batchSig);

    const claimSig = await program.methods
      .claimWinnings(bn(4_000_000), bn(3_200_000), proofs[1].map((node) => [...node]))
      .accounts({
        platformConfig,
        arena,
        userAccount: userPda(userB.publicKey),
        position: positionPda(arena, userB.publicKey),
        tusdcMint,
        userTusdc: tusdcAta(userB.publicKey),
        user: userB.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .signers([userB])
      .rpc();
    await confirm(claimSig);

    let arenaAccount = await program.account.arenaAccount.fetch(arena);
    assert.equal(arenaAccount.totalClaimedPayout.toNumber(), 7_200_000);
    assert.equal(arenaAccount.resolvedCount, 2);

    const userAAccount = await program.account.userAccount.fetch(userPda(userA.publicKey));
    const userBAccount = await program.account.userAccount.fetch(userPda(userB.publicKey));
    const creatorAccount = await program.account.userAccount.fetch(userPda(authority.publicKey));
    assert.equal(userAAccount.totalBalance.toNumber(), 10_000_000);
    assert.equal(userBAccount.totalBalance.toNumber(), 9_200_000);
    assert.equal(creatorAccount.inPlayDebt.toNumber(), 5_000_000);
    assert.equal(await getAtaAmount(userA.publicKey), 10_000_000);
    assert.equal(await getAtaAmount(userB.publicKey), 9_200_000);

    const finalizeSig = await program.methods
      .finalizeArena()
      .accounts({
        platformConfig,
        arena,
        creator: authority.publicKey,
        creatorAccount: userPda(authority.publicKey),
        creatorTusdc: tusdcAta(authority.publicKey),
        treasuryAccount: userPda(treasury.publicKey),
        treasuryWallet: treasury.publicKey,
        tusdcMint,
        treasuryTusdc: tusdcAta(treasury.publicKey),
        payer: intruder.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([intruder])
      .rpc();
    await confirm(finalizeSig);

    arenaAccount = await program.account.arenaAccount.fetch(arena);
    const treasuryAccount = await program.account.userAccount.fetch(userPda(treasury.publicKey));
    const creatorFinalAccount = await program.account.userAccount.fetch(userPda(authority.publicKey));
    assert.deepEqual(arenaAccount.status, { finalized: {} });
    assert.equal(treasuryAccount.totalBalance.toNumber(), 20_800_000);
    assert.equal(await getAtaAmount(treasury.publicKey), 20_800_000);
    assert.equal(creatorFinalAccount.totalBalance.toNumber(), 20_000_000);
    assert.equal(creatorFinalAccount.inPlayDebt.toNumber(), 0);
    assert.equal(await getAtaAmount(authority.publicKey), 20_000_000);
  });

  it("rejects settlement roots that try to pay guarantee plus net entries", async () => {
    const creator = await createUser(10_000_000);
    const user = await createUser(10_000_000);
    const arenaId = bytes32Buffer("settle-cap");
    const now = Math.floor(Date.now() / 1000);
    const arena = await createArena({
      arenaId,
      creator: creator.publicKey,
      creatorSigner: creator,
      entryFee: 4_000_000,
      feeBps: 1_000,
      guaranteedPrizeTarget: 5_000_000,
      startTime: now + 2,
      endTime: now + 4,
    });

    await joinArena({ arena, user });
    await sleep(4200);

    const leaf = merkleLeaf(arenaId, 1, user.publicKey, 4_000_000, 8_600_000);
    const { root, proofs } = buildMerkle([leaf]);

    await program.methods
      .postSettlementRoot([...root])
      .accounts({
        platformConfig,
        arena,
        authority: arenaOperator.publicKey,
      })
      .signers([arenaOperator])
      .rpc();

    await sleep(2500);

    await expectFailure(
      program.methods
        .settleArenaBatch([
          {
            lockedAmount: bn(4_000_000),
            payoutAmount: bn(8_600_000),
            proof: proofs[0].map((node) => [...node]),
          },
        ])
        .accounts({
          platformConfig,
          arena,
          authority: arenaOperator.publicKey,
          tusdcMint,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .remainingAccounts([
          { pubkey: userPda(user.publicKey), isWritable: true, isSigner: false },
          { pubkey: tusdcAta(user.publicKey), isWritable: true, isSigner: false },
          { pubkey: positionPda(arena, user.publicKey), isWritable: true, isSigner: false },
        ])
        .signers([arenaOperator])
        .rpc(),
      "ArenaPoolExceeded"
    );
  });

  it("cancels arenas with admin batch refunds, user fallback refunds, and releases the creator guarantee lock", async () => {
    const userA = await createUser(3_000_000);
    const userB = await createUser(3_000_000);
    const arenaId = bytes32Buffer("cancel-arena");
    const now = Math.floor(Date.now() / 1000);
    const creatorBefore = await getAtaAmount(authority.publicKey);
    const arena = await createArena({
      arenaId,
      entryFee: 500_000,
      feeBps: 500,
      guaranteedPrizeTarget: 1_000_000,
      startTime: now + 60,
      endTime: now + 120,
    });

    const treasuryBefore = await getAtaAmount(treasury.publicKey);
    assert.equal(await getAtaAmount(authority.publicKey), creatorBefore - 1_000_000);
    await joinArena({ arena, user: userA });
    await joinArena({ arena, user: userB });

    await program.methods
      .cancelArena()
      .accounts({
        platformConfig,
        arena,
        authority: arenaOperator.publicKey,
      })
      .signers([arenaOperator])
      .rpc();

    const refundBatchSig = await program.methods
      .refundArenaBatch()
      .accounts({
        platformConfig,
        arena,
        authority: arenaOperator.publicKey,
        tusdcMint,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .remainingAccounts([
        { pubkey: userPda(userA.publicKey), isWritable: true, isSigner: false },
        { pubkey: tusdcAta(userA.publicKey), isWritable: true, isSigner: false },
        { pubkey: positionPda(arena, userA.publicKey), isWritable: true, isSigner: false },
      ])
      .signers([arenaOperator])
      .rpc();
    await confirm(refundBatchSig);

    const refundSig = await program.methods
      .claimRefund()
      .accounts({
        platformConfig,
        arena,
        userAccount: userPda(userB.publicKey),
        position: positionPda(arena, userB.publicKey),
        tusdcMint,
        userTusdc: tusdcAta(userB.publicKey),
        user: userB.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .signers([userB])
      .rpc();
    await confirm(refundSig);

    const cancelFinalizeSig = await program.methods
      .finalizeArena()
      .accounts({
        platformConfig,
        arena,
        creator: authority.publicKey,
        creatorAccount: userPda(authority.publicKey),
        creatorTusdc: tusdcAta(authority.publicKey),
        treasuryAccount: userPda(treasury.publicKey),
        treasuryWallet: treasury.publicKey,
        tusdcMint,
        treasuryTusdc: tusdcAta(treasury.publicKey),
        payer: intruder.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([intruder])
      .rpc();
    await confirm(cancelFinalizeSig);

    const arenaAccount = await program.account.arenaAccount.fetch(arena);
    const treasuryAccount = await program.account.userAccount.fetch(userPda(treasury.publicKey));
    const creatorAccount = await program.account.userAccount.fetch(userPda(authority.publicKey));
    assert.deepEqual(arenaAccount.status, { finalized: {} });
    assert.equal(treasuryAccount.totalBalance.toNumber(), 20_800_000);
    assert.equal(await getAtaAmount(treasury.publicKey), treasuryBefore);
    assert.equal(creatorAccount.inPlayDebt.toNumber(), 0);
    assert.equal(await getAtaAmount(authority.publicKey), creatorBefore);
    assert.equal(await getAtaAmount(userA.publicKey), 3_000_000);
    assert.equal(await getAtaAmount(userB.publicKey), 3_000_000);
  });

  it("burns withdrawals with nonce protection once balances are unlocked", async () => {
    const user = await createUser(1_500_000);

    await program.methods
      .withdrawRequest(bn(500_000), bn(1))
      .accounts({
        user: user.publicKey,
        userAccount: userPda(user.publicKey),
        tusdcMint,
        userTusdc: tusdcAta(user.publicKey),
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();

    await expectFailure(
      program.methods
        .withdrawRequest(bn(1), bn(1))
        .accounts({
          user: user.publicKey,
          userAccount: userPda(user.publicKey),
          tusdcMint,
          userTusdc: tusdcAta(user.publicKey),
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        })
        .signers([user])
        .rpc(),
    );

    const userAccount = await program.account.userAccount.fetch(userPda(user.publicKey));
    assert.equal(userAccount.totalBalance.toNumber(), 1_000_000);
    assert.equal(userAccount.lastNonce.toNumber(), 1);
    assert.equal(await getAtaAmount(user.publicKey), 1_000_000);
  });
});
