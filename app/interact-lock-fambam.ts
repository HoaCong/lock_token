import * as anchor from "@coral-xyz/anchor";
import { getAssociatedTokenAddress, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { clusterApiUrl, Connection, Keypair, PublicKey } from "@solana/web3.js";
import IDL from "./token_lock.json";

const connection = new Connection(clusterApiUrl("devnet"), "confirmed");
const secretKey = JSON.parse(
  "[51,80,45,229,107,0,220,224,22,221,147,223,14,109,37,94,81,97,142,150,25,1,129,19,178,184,179,66,197,109,202,137,94,199,143,63,33,94,206,1,35,0,124,12,93,1,235,182,241,131,95,215,7,154,115,113,134,183,228,32,198,1,129,181]"
);
const keypair = Keypair.fromSecretKey(Uint8Array.from(secretKey));
const wallet = new anchor.Wallet(keypair);
const provider = new anchor.AnchorProvider(connection, wallet, {
  commitment: "confirmed",
});
anchor.setProvider(provider);
const program = new anchor.Program(IDL as anchor.Idl, provider);
console.log(`Program ID: ${program.programId.toBase58()}`);

async function initialize() {
  const [adminSettingsPda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from("admin_settings")],
    program.programId
  );

  const tx = await program.methods
    .initialize()
    .accounts({
      adminSettings: adminSettingsPda,
      admin: wallet.publicKey,
    })
    .rpc();
  console.log(`Transaction successful: ${tx}`);
}

async function addTokenSupported(tokenMint: PublicKey) {
  const [adminSettingsPda, _] = PublicKey.findProgramAddressSync(
    [Buffer.from("admin_settings")],
    program.programId
  );
  const [supportedTokenPda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from("supported_token"), tokenMint.toBuffer()],
    program.programId
  );

  const tx = await program.methods
    .addSupportedToken()
    .accounts({
      supportedToken: supportedTokenPda,
      mint: tokenMint,
      adminSettings: adminSettingsPda,
      admin: wallet.publicKey,
    })
    .rpc();
  console.log(`Token ${tokenMint.toBase58()} added successfully: ${tx}`);
}

async function lockToken(mint: PublicKey, amountDecimal: number) {
  const amount = new anchor.BN(amountDecimal);
  console.log(`Locking ${amount} token...`);

  const [adminSettingsPda, _] = PublicKey.findProgramAddressSync(
    [Buffer.from("admin_settings")],
    program.programId
  );
  const [supportedTokenPda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from("supported_token"), mint.toBuffer()],
    program.programId
  );
  const [userLockPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("user_lock"), wallet.publicKey.toBuffer(), mint.toBuffer()],
    program.programId
  );

  // const [lockTokenAccount] = PublicKey.findProgramAddressSync(
  //     [Buffer.from("lock_token_account"), wallet.publicKey.toBuffer(), mint.toBuffer()],
  //     program.programId
  // );
  const lockTokenAccount = Keypair.generate();
  console.log(`Lock Token Account: ${lockTokenAccount.publicKey.toBase58()}`);

  const userTokenAccount = await getAssociatedTokenAddress(
    mint,
    wallet.publicKey
  );
  console.log(`User Token Account: ${userTokenAccount.toBase58()}`);
  // const lockTokenAccount = await getAssociatedTokenAddress(mint, userLockPda);
  // const lockTokenAccount = await getAccountAddressForLock(wallet.publicKey, mint);
  // console.log(`Lock Token Account: ${lockTokenAccount.toBase58()}`);

  const tx = await program.methods
    .lockTokens(amount)
    .accounts({
      userLock: userLockPda,
      supportedToken: supportedTokenPda,
      adminSettings: adminSettingsPda,
      mint: mint,
      userTokenAccount,
      lockTokenAccount: lockTokenAccount.publicKey,
      user: wallet.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    })
    .signers([lockTokenAccount])
    .rpc();
  console.log(`Locked ${amount} token successfully: ${tx}`);
}

(async () => {
  // await initialize();
  const tokenMint = new PublicKey(
    "HqvymBuwH4pwwUffEVqK6VP6FHzFaHrekFv1vscspXq3"
  );
  // await addTokenSupported(tokenMint);

  await lockToken(tokenMint, 100_000_000_000); // 100 token`
})();
