import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { SolTokenManager } from "../target/types/sol_token_manager";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, createMint, createAccount, mintTo } from "@solana/spl-token";
import { assert } from "chai";

describe("sol_token_manager", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SolTokenManager as Program<SolTokenManager>;

  let programState: Keypair;
  let programVault: PublicKey;
  let programVaultBump: number;
  let admin: Keypair;
  let user: Keypair;

  let wsolMint: PublicKey;
  let usdcMint: PublicKey;
  let poolKeypair: Keypair;
  let poolStateKeypair: Keypair;
  let wsolVault: PublicKey;
  let usdcVault: PublicKey;
  let tickArray0: PublicKey;
  let tickArray1: PublicKey;
  let tickArray2: PublicKey;
  let oracle: PublicKey;
  let userUsdcAccount: PublicKey;

  before(async () => {
    admin = Keypair.generate();
    user = Keypair.generate();
    programState = Keypair.generate();

    // Airdrop SOL to admin and user
    await provider.connection.requestAirdrop(admin.publicKey, 10 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(user.publicKey, 10 * LAMPORTS_PER_SOL);

    // Find program vault PDA
    [programVault, programVaultBump] = await PublicKey.findProgramAddress(
      [Buffer.from("program_vault")],
      program.programId
    );

    // Create mock tokens and pool for testing
    wsolMint = await createMint(provider.connection, admin, admin.publicKey, null, 9);
    usdcMint = await createMint(provider.connection, admin, admin.publicKey, null, 6);

    poolKeypair = Keypair.generate();
    poolStateKeypair = Keypair.generate();
    wsolVault = await createAccount(provider.connection, admin, wsolMint, poolKeypair.publicKey);
    usdcVault = await createAccount(provider.connection, admin, usdcMint, poolKeypair.publicKey);
    
    // Mock addresses for tick arrays and oracle
    tickArray0 = Keypair.generate().publicKey;
    tickArray1 = Keypair.generate().publicKey;
    tickArray2 = Keypair.generate().publicKey;
    oracle = Keypair.generate().publicKey;

    // Create user's USDC account
    userUsdcAccount = await createAccount(provider.connection, user, usdcMint, user.publicKey);
  });

  it("Initializes the program", async () => {
    await program.methods.initialize(admin.publicKey)
      .accounts({
        programState: programState.publicKey,
        user: admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin, programState])
      .rpc();

    const state = await program.account.programState.fetch(programState.publicKey);
    assert.ok(state.admin.equals(admin.publicKey));
    assert.ok(state.programVault.equals(programVault));
    assert.equal(state.programVaultBump, programVaultBump);
  });

  it("Deposits SOL", async () => {
    const depositAmount = new anchor.BN(1 * LAMPORTS_PER_SOL);
    const initialBalance = await provider.connection.getBalance(programVault);

    await program.methods.depositSol(depositAmount)
      .accounts({
        user: user.publicKey,
        programVault: programVault,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    const finalBalance = await provider.connection.getBalance(programVault);
    assert.equal(finalBalance - initialBalance, depositAmount.toNumber());
  });

  it("Withdraws SOL (admin only)", async () => {
    const withdrawAmount = new anchor.BN(0.5 * LAMPORTS_PER_SOL);
    const initialBalance = await provider.connection.getBalance(admin.publicKey);

    await program.methods.withdrawSol(withdrawAmount)
      .accounts({
        user: admin.publicKey,
        programState: programState.publicKey,
        programVault: programVault,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const finalBalance = await provider.connection.getBalance(admin.publicKey);
    assert.equal(finalBalance - initialBalance, withdrawAmount.toNumber());
  });

  it("Fails to withdraw SOL (non-admin)", async () => {
    const withdrawAmount = new anchor.BN(0.5 * LAMPORTS_PER_SOL);

    try {
      await program.methods.withdrawSol(withdrawAmount)
        .accounts({
          user: user.publicKey,
          programState: programState.publicKey,
          programVault: programVault,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();
      assert.fail("Expected an error");
    } catch (error) {
      assert.include(error.message, "You are not authorized to perform this action");
    }
  });

  it("Buys tokens", async () => {
    // For this test, we'll mock the Raydium swap by directly minting USDC to the program's vault
    const amountIn = new anchor.BN(1 * LAMPORTS_PER_SOL);
    const minimumAmountOut = new anchor.BN(1000000); // 1 USDC

    // Mint some USDC to the program's vault to simulate a successful swap
    await mintTo(
      provider.connection,
      admin,
      usdcMint,
      usdcVault,
      admin,
      1000000 // 1 USDC
    );

    const initialUserUsdcBalance = (await provider.connection.getTokenAccountBalance(userUsdcAccount)).value.amount;

    await program.methods.buyTokens(amountIn, minimumAmountOut)
      .accounts({
        programState: programState.publicKey,
        programVault: programVault,
        pool: poolKeypair.publicKey,
        poolState: poolStateKeypair.publicKey,
        wsolVault: wsolVault,
        usdcVault: usdcVault,
        tickArray0: tickArray0,
        tickArray1: tickArray1,
        tickArray2: tickArray2,
        oracle: oracle,
        userUsdc: userUsdcAccount,
        raydiumProgram: program.programId, // Mock Raydium program ID for testing
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const finalUserUsdcBalance = (await provider.connection.getTokenAccountBalance(userUsdcAccount)).value.amount;
    assert.equal(Number(finalUserUsdcBalance) - Number(initialUserUsdcBalance), 1000000);
  });
});