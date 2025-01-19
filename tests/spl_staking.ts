import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SplStaking } from "../target/types/spl_staking";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getAccount,
} from "@solana/spl-token";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import { join } from "path";
import { readFileSync } from "fs";
import { json } from "stream/consumers";
import assert from "assert";
import { program } from "@coral-xyz/anchor/dist/cjs/native/system";
import { BN } from "bn.js";
import { expect } from "chai";
import { log } from "console";

describe("spl_staking", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SplStaking as Program<SplStaking>;

  const wallet = provider.wallet;

  const WALLET_PATH = join(process.env["HOME"]!, ".config/solana/id.json");
  console.log(
    `🚀 ~ file: spl_staking.ts:20 ~ describe ~ WALLET_PATH:`,
    WALLET_PATH
  );

  const owner = Keypair.fromSecretKey(
    Buffer.from(JSON.parse(readFileSync(WALLET_PATH, { encoding: "utf-8" })))
  );
  console.log(
    `🚀 ~ file: spl_staking.ts:24 ~ describe ~ admin:`,
    owner.publicKey
  );

  // const owner = Keypair.generate();
  const user = Keypair.generate();
  let mint: PublicKey;
  let userTokenAccount: PublicKey;
  let vault: PublicKey;
  let state: PublicKey;
  let userStakeAccount: PublicKey;
  let vaultBump:number;

  // Test constants
  const INITIAL_MINT_AMOUNT = 1_000_000_000; // 1000 tokens with 6 decimals
  const STAKE_AMOUNT = 100_000_000; // 100 tokens
  const APY = 20; // 20%

  before(async () => {
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        owner.publicKey,
        10 * LAMPORTS_PER_SOL
      ),
      "confirmed"
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        user.publicKey,
        10 * LAMPORTS_PER_SOL
      ),
      "confirmed"
    );

    mint = await createMint(
      provider.connection,
      owner,
      owner.publicKey,
      null,
      6,
      Keypair.generate(),
      undefined,
      TOKEN_PROGRAM_ID
    );

    userTokenAccount = await createAccount(
      provider.connection,
      user,
      mint,
      user.publicKey,
      Keypair.generate(),
      undefined,
      TOKEN_PROGRAM_ID
    );

      // vault = await createAccount(
      //   provider.connection,
      //   owner,
      //   mint,
      //   owner.publicKey,
      //   Keypair.generate(),
      //   undefined,
      //   TOKEN_PROGRAM_ID
      // );

    await mintTo(
      provider.connection,
      owner,
      mint,
      userTokenAccount,
      owner,
      INITIAL_MINT_AMOUNT
    );

    const [stateAddr] = PublicKey.findProgramAddressSync(
      [Buffer.from("state")],
      program.programId
    );
    state = stateAddr;

    const [vaultAddr, vBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault")],
      program.programId
    );
    vault = vaultAddr;
    console.log(`🚀 ~ file: spl_staking.ts:130 ~ before ~ vault:`, vault.toString());

    vaultBump = vBump;


    const [userStakeAddr] = PublicKey.findProgramAddressSync(
      [user.publicKey.toBuffer()],
      program.programId
    );
    userStakeAccount = userStakeAddr;


  });

  it("Is initialized!", async () => {
    const now = Math.floor(Date.now() / 1000);
    const startTime = now - 100; // Started 100 seconds ago
    const endTime = now + 31536000; // Ends in 1 year
    const lockDuration = 3; 

    console.log("state", state.toString());
    console.log("owner", owner.publicKey.toString());
    
    



    await program.methods
    .initializeState(
      new anchor.BN(startTime),
      new anchor.BN(endTime),
      new anchor.BN(lockDuration),
      new anchor.BN(APY)
    )
    .accounts({
      state,
      owner: owner.publicKey,
      vault,
      tokenProgram:TOKEN_PROGRAM_ID,
      mint,
      systemProgram: SystemProgram.programId,
    })
    .signers([owner])
    .rpc();


    const stateAccount = await program.account.state.fetch(state);
    expect(stateAccount.owner.toString()).to.equal(owner.publicKey.toString());
    expect(stateAccount.startTime.toNumber()).to.equal(startTime);
    expect(stateAccount.endTime.toNumber()).to.equal(endTime);
    expect(stateAccount.lockDuration.toNumber()).to.equal(lockDuration);
    expect(stateAccount.apy.toNumber()).to.equal(APY);

   
  });


  it("it stakes token", async()=>{
    const initialUserBalance = (await getAccount(provider.connection, userTokenAccount)).amount;
    const initialVaultBalance = (await getAccount(provider.connection, vault)).amount;

    await program.methods
    .stake(new anchor.BN(STAKE_AMOUNT))
    .accounts({
      state,
      userStakeAccount,
      userTokenAccount,
      vault,
      tokenProgram: TOKEN_PROGRAM_ID,
      user: user.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .signers([user])
    .rpc();

    const finalUserBalance = (await getAccount(provider.connection, userTokenAccount)).amount;
    const finalVaultBalance = (await getAccount(provider.connection, vault)).amount;
    console.log(`🚀 ~ file: spl_staking.ts:207 ~ it ~ finalVaultBalance:`, finalVaultBalance);

    expect(finalUserBalance.toString()).to.equal(
      (initialUserBalance - BigInt(STAKE_AMOUNT)).toString()
    );
    expect(finalVaultBalance.toString()).to.equal(
      (initialVaultBalance + BigInt(STAKE_AMOUNT)).toString()
    );

    const userStakeData = await program.account.userStakeAccount.fetch(userStakeAccount);
    expect(userStakeData.owner.toString()).to.equal(user.publicKey.toString());
    expect(userStakeData.amountStaked.toNumber()).to.equal(STAKE_AMOUNT);

  })

  it("it can not stake twice",async()=>{

    try {


      await program.methods
        .stake(new anchor.BN(STAKE_AMOUNT))
        .accounts({
          state,
          userStakeAccount,
          userTokenAccount,
          vault,
          tokenProgram: TOKEN_PROGRAM_ID,
          user: user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();
      expect.fail("Should have thrown error");
      
    } catch (error) {

      expect(error.toString()).to.include("AlreadyStaked");
      
    }
  })

  it ("it claims reward", async()=>{
    await new Promise((resolve) => setTimeout(resolve, 5000));

    const initialUserBalance = (await getAccount(provider.connection, userTokenAccount)).amount;

    await program.methods
    .claimRewards()
    .accounts({
      state,
      userStakeAccount,
      userTokenAccount,
      vault,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .rpc();

    const finalUserBalance = (await getAccount(provider.connection, userTokenAccount)).amount;
    expect(finalUserBalance > initialUserBalance).to.be.true;
    
    const userStakeData = await program.account.userStakeAccount.fetch(userStakeAccount);
    expect(userStakeData.rewardClaimed.toNumber()).to.be.greaterThan(0);
  
  })

  it ("it cannot unstake before time", async()=>{
    try {

      await program.methods
        .unstake()
        .accounts({
          state,
          userStakeAccount,
          userTokenAccount,
          vault,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();
      expect.fail("Should have thrown error");
      
    } catch (error) {
      expect(error.toString()).to.include("The lock period has not yet ended..");
    }
  })

  it ("it unstakes after lock period", async()=>{
    await new Promise((resolve) => setTimeout(resolve, 3000)); //

    const initialUserBalance = (await getAccount(provider.connection, userTokenAccount)).amount;
    const initialVaultBalance = (await getAccount(provider.connection, vault)).amount;


    await program.methods
    .unstake()
    .accounts({
      state,
      userStakeAccount,
      userTokenAccount,
      vault,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .rpc();

    const finalUserBalance = (await getAccount(provider.connection, userTokenAccount)).amount;
    const finalVaultBalance = (await getAccount(provider.connection, vault)).amount;
    
    expect(finalUserBalance > initialUserBalance).to.be.true;
    expect(finalVaultBalance < initialVaultBalance).to.be.true;

    const userStakeData = await program.account.userStakeAccount.fetch(userStakeAccount);
    expect(userStakeData.amountStaked.toNumber()).to.equal(0);


  })
});
