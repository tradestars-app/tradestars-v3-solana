import * as anchor from "@coral-xyz/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  getAccount,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import crypto from "crypto";
import fs from "fs";
import os from "os";
import path from "path";

const { BN } = anchor.default;

const DEVNET_RPC = process.env.ANCHOR_PROVIDER_URL ?? "https://api.devnet.solana.com";
const WALLET_PATH =
  process.env.ANCHOR_WALLET ??
  path.join(os.homedir(), ".config", "solana", "id.json");

function readWallet(filePath) {
  return Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(filePath, "utf8"))),
  );
}

const idl = JSON.parse(
  fs.readFileSync("target/idl/tradestars_arena.json", "utf8"),
);
const PROGRAM_ID = new PublicKey(idl.address);
const connection = new Connection(DEVNET_RPC, "confirmed");
const wallet = new anchor.Wallet(readWallet(WALLET_PATH));
const provider = new anchor.AnchorProvider(connection, wallet, {
  commitment: "confirmed",
});
anchor.setProvider(provider);

const program = new anchor.Program(idl, provider);
if (!program.programId.equals(PROGRAM_ID)) {
  throw new Error(
    `IDL program id mismatch: expected ${PROGRAM_ID.toBase58()}, got ${program.programId.toBase58()}`,
  );
}

const [platformConfig] = PublicKey.findProgramAddressSync(
  [Buffer.from("config")],
  program.programId,
);
const [tusdcMint] = PublicKey.findProgramAddressSync(
  [Buffer.from("tusdc_mint")],
  program.programId,
);

const config = await program.account.platformConfig.fetch(platformConfig);
if (!config.mintingAuthority.equals(wallet.publicKey)) {
  throw new Error(
    `Wallet ${wallet.publicKey.toBase58()} is not the configured minting authority ${config.mintingAuthority.toBase58()}`,
  );
}

const user = Keypair.generate().publicKey;
const amount = Number(process.env.SMOKE_DEPOSIT_AMOUNT ?? 12_345_678);
const baseTxHash = crypto.randomBytes(32);
const logIndex = Math.floor(Math.random() * 100_000);
const logIndexLe = Buffer.alloc(4);
logIndexLe.writeUInt32LE(logIndex);

const [userAccount] = PublicKey.findProgramAddressSync(
  [Buffer.from("user"), user.toBuffer()],
  program.programId,
);
const [depositMarker] = PublicKey.findProgramAddressSync(
  [Buffer.from("deposit"), baseTxHash, logIndexLe],
  program.programId,
);
const userTusdc = getAssociatedTokenAddressSync(
  tusdcMint,
  user,
  false,
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
);

const signature = await program.methods
  .depositCollateral(new BN(amount), [...baseTxHash], logIndex)
  .accounts({
    platformConfig,
    tusdcMint,
    user,
    userAccount,
    depositMarker,
    userTusdc,
    mintingAuthority: wallet.publicKey,
    tokenProgram: TOKEN_2022_PROGRAM_ID,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  })
  .rpc();

const userState = await program.account.userAccount.fetch(userAccount);
const tokenState = await getAccount(
  connection,
  userTusdc,
  "confirmed",
  TOKEN_2022_PROGRAM_ID,
);

console.log(
  JSON.stringify(
    {
      program: program.programId.toBase58(),
      platformConfig: platformConfig.toBase58(),
      tusdcMint: tusdcMint.toBase58(),
      mintingAuthority: wallet.publicKey.toBase58(),
      user: user.toBase58(),
      userAccount: userAccount.toBase58(),
      userTusdc: userTusdc.toBase58(),
      depositMarker: depositMarker.toBase58(),
      amount: amount.toString(),
      baseTxHash: `0x${baseTxHash.toString("hex")}`,
      logIndex,
      signature,
      ledgerBalance: userState.totalBalance.toString(),
      tokenBalance: tokenState.amount.toString(),
    },
    null,
    2,
  ),
);
