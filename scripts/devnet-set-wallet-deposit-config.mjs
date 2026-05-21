import * as anchor from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import fs from "fs";
import os from "os";
import path from "path";

const DEVNET_RPC = process.env.ANCHOR_PROVIDER_URL ?? "https://api.devnet.solana.com";
const WALLET_PATH =
  process.env.ANCHOR_WALLET ??
  path.join(os.homedir(), ".config", "solana", "id.json");
const USDC_MINT = process.env.USDC_MINT;

if (!USDC_MINT) {
  throw new Error("USDC_MINT env var is required");
}

function readWallet(filePath) {
  return Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(filePath, "utf8"))),
  );
}

const idl = JSON.parse(
  fs.readFileSync("target/idl/tradestars_arena.json", "utf8"),
);
const PROGRAM_ID = new PublicKey("2YEsWGLfhsUwDWoFCEZQQeES8KN9jHHRXLkbtwoDQGV8");
idl.address = PROGRAM_ID.toBase58();
const wallet = new anchor.Wallet(readWallet(WALLET_PATH));
const connection = new anchor.web3.Connection(DEVNET_RPC, "confirmed");
const provider = new anchor.AnchorProvider(connection, wallet, {
  commitment: "confirmed",
});
anchor.setProvider(provider);

const program = new anchor.Program(idl, provider);
const [platformConfig] = PublicKey.findProgramAddressSync(
  [Buffer.from("config")],
  program.programId,
);
const [walletDepositConfig] = PublicKey.findProgramAddressSync(
  [Buffer.from("wallet_deposit_config")],
  program.programId,
);

const platform = await program.account.platformConfig.fetch(platformConfig);
if (!platform.authority.equals(wallet.publicKey)) {
  throw new Error(
    `Wallet ${wallet.publicKey.toBase58()} is not the platform authority ${platform.authority.toBase58()}`,
  );
}

const usdcMint = new PublicKey(USDC_MINT);
const signature = await program.methods
  .setWalletDepositConfig(usdcMint)
  .accounts({
    platformConfig,
    walletDepositConfig,
    authority: wallet.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();

const config = await program.account.walletDepositConfig.fetch(walletDepositConfig);

console.log(
  JSON.stringify(
    {
      program: program.programId.toBase58(),
      platformConfig: platformConfig.toBase58(),
      walletDepositConfig: walletDepositConfig.toBase58(),
      authority: wallet.publicKey.toBase58(),
      usdcMint: config.usdcMint.toBase58(),
      signature,
    },
    null,
    2,
  ),
);
