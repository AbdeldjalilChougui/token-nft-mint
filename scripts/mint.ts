/**
 * Mint a fungible token or an NFT through the deployed token_nft_mint program.
 *
 *   pnpm mint token --name "Test Dinar" --symbol TDZD --uri https://... --decimals 6 --amount 1000
 *   pnpm mint nft   --name "Sahara #1" --symbol ART --uri https://.../1.json
 *
 * Options: --cluster devnet|localnet|<url> (default devnet), --keypair <path>
 * (default ~/.config/solana/id.json), --idl <path> (default target/idl/token_nft_mint.json).
 *
 * Devnet/localnet only: mainnet URLs are refused. The wallet's secret key is
 * read from disk to sign and is never printed.
 */
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import {
  Connection,
  Keypair,
  PublicKey,
  SYSVAR_RENT_PUBKEY,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";

const METADATA_PROGRAM = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
const CLUSTERS: Record<string, string> = { devnet: "https://api.devnet.solana.com", localnet: "http://127.0.0.1:8899" };

type Idl = { address: string; instructions: { name: string; discriminator: number[] }[] };

function arg(name: string, def?: string): string {
  const i = process.argv.indexOf(`--${name}`);
  const v = i >= 0 ? process.argv[i + 1] : def;
  if (v === undefined) throw new Error(`missing --${name}`);
  return v;
}

/** Tiny Borsh writer: just what these instructions need. */
class Writer {
  parts: Buffer[] = [];
  u8(v: number) {
    this.parts.push(Buffer.from([v]));
    return this;
  }
  u64(v: bigint) {
    const b = Buffer.alloc(8);
    b.writeBigUInt64LE(v);
    this.parts.push(b);
    return this;
  }
  string(s: string) {
    const b = Buffer.from(s, "utf8");
    const len = Buffer.alloc(4);
    len.writeUInt32LE(b.length);
    this.parts.push(len, b);
    return this;
  }
  bytes(b: number[]) {
    this.parts.push(Buffer.from(b));
    return this;
  }
  done() {
    return Buffer.concat(this.parts);
  }
}

const metadataPda = (mint: PublicKey) =>
  PublicKey.findProgramAddressSync([Buffer.from("metadata"), METADATA_PROGRAM.toBuffer(), mint.toBuffer()], METADATA_PROGRAM)[0];
const editionPda = (mint: PublicKey) =>
  PublicKey.findProgramAddressSync([Buffer.from("metadata"), METADATA_PROGRAM.toBuffer(), mint.toBuffer(), Buffer.from("edition")], METADATA_PROGRAM)[0];
const meta = (pubkey: PublicKey, isSigner: boolean, isWritable: boolean) => ({ pubkey, isSigner, isWritable });

async function main() {
  const mode = process.argv[2];
  if (mode !== "token" && mode !== "nft") throw new Error("usage: mint.ts token|nft --name ... --symbol ... --uri ...");
  const cluster = arg("cluster", "devnet");
  const url = CLUSTERS[cluster] ?? cluster;
  if (/mainnet/i.test(url)) throw new Error("refusing to run against mainnet");
  const conn = new Connection(url, "confirmed");
  if ((await conn.getGenesisHash()) === "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d") throw new Error("refusing: RPC is mainnet-beta");

  const payer = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(arg("keypair", `${homedir()}/.config/solana/id.json`), "utf8"))));
  const idl = JSON.parse(readFileSync(arg("idl", "target/idl/token_nft_mint.json"), "utf8")) as Idl;
  const programId = new PublicKey(idl.address);
  const disc = (n: string) => idl.instructions.find((i) => i.name === n)!.discriminator;
  const mint = Keypair.generate(); // the new mint's own key, used once to create it
  const [name, symbol, uri] = [arg("name"), arg("symbol"), arg("uri")];
  const ixs: TransactionInstruction[] = [];

  if (mode === "token") {
    const decimals = Number(arg("decimals", "6"));
    const amount = BigInt(Math.round(Number(arg("amount", "0")) * 10 ** decimals));
    ixs.push(
      new TransactionInstruction({
        programId,
        keys: [
          meta(payer.publicKey, true, true),
          meta(mint.publicKey, true, true),
          meta(metadataPda(mint.publicKey), false, true),
          meta(METADATA_PROGRAM, false, false),
          meta(TOKEN_PROGRAM_ID, false, false),
          meta(SystemProgram.programId, false, false),
          meta(SYSVAR_RENT_PUBKEY, false, false),
        ],
        data: new Writer().bytes(disc("create_token")).u8(decimals).string(name).string(symbol).string(uri).done(),
      }),
    );
    if (amount > 0n) {
      ixs.push(
        new TransactionInstruction({
          programId,
          keys: [
            meta(payer.publicKey, true, true),
            meta(mint.publicKey, false, true),
            meta(payer.publicKey, false, false),
            meta(getAssociatedTokenAddressSync(mint.publicKey, payer.publicKey), false, true),
            meta(ASSOCIATED_TOKEN_PROGRAM_ID, false, false),
            meta(TOKEN_PROGRAM_ID, false, false),
            meta(SystemProgram.programId, false, false),
          ],
          data: new Writer().bytes(disc("mint_tokens")).u64(amount).done(),
        }),
      );
    }
  } else {
    ixs.push(
      new TransactionInstruction({
        programId,
        keys: [
          meta(payer.publicKey, true, true),
          meta(mint.publicKey, true, true),
          meta(getAssociatedTokenAddressSync(mint.publicKey, payer.publicKey), false, true),
          meta(metadataPda(mint.publicKey), false, true),
          meta(editionPda(mint.publicKey), false, true),
          meta(METADATA_PROGRAM, false, false),
          meta(ASSOCIATED_TOKEN_PROGRAM_ID, false, false),
          meta(TOKEN_PROGRAM_ID, false, false),
          meta(SystemProgram.programId, false, false),
          meta(SYSVAR_RENT_PUBKEY, false, false),
        ],
        data: new Writer().bytes(disc("create_nft")).string(name).string(symbol).string(uri).done(),
      }),
    );
  }

  const sig = await sendAndConfirmTransaction(conn, new Transaction().add(...ixs), [payer, mint]);
  console.log(`mint:      ${mint.publicKey.toBase58()}`);
  console.log(`metadata:  ${metadataPda(mint.publicKey).toBase58()}`);
  if (mode === "nft") console.log(`edition:   ${editionPda(mint.publicKey).toBase58()}`);
  console.log(`signature: ${sig}`);
  const q = CLUSTERS[cluster] && cluster !== "localnet" ? `cluster=${cluster}` : `cluster=custom&customUrl=${encodeURIComponent(url)}`;
  console.log(`explorer:  https://explorer.solana.com/address/${mint.publicKey.toBase58()}?${q}`);
}

main().catch((e) => {
  console.error(`error: ${e.message}`);
  process.exit(1);
});
