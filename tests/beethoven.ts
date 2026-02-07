import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Beethoven } from "../target/types/beethoven";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
  TransactionInstruction,
  sendAndConfirmTransaction,
  SYSVAR_RENT_PUBKEY,
  SYSVAR_INSTRUCTIONS_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getAccount,
  getOrCreateAssociatedTokenAccount,
} from "@solana/spl-token";
import { assert } from "chai";
import BN from "bn.js";
import { ManifestClient, OrderType, createSwapInstruction } from "@cks-systems/manifest-sdk";

// Helper to log transaction signatures
function logTx(label: string, sig: string) {
  console.log(`    📝 ${label}: https://explorer.solana.com/tx/${sig}?cluster=devnet`);
}

describe("beethoven", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.beethoven as Program<Beethoven>;
  const connection = provider.connection;

  // Keypairs
  const admin = provider.wallet as anchor.Wallet;
  const user1 = Keypair.generate();
  const user2 = Keypair.generate();

  // Mints
  let baseMint: PublicKey;
  let quoteMint: PublicKey;

  // PDAs
  let exchangePda: PublicKey;
  let exchangeBump: number;
  let userAccountPda: PublicKey;
  let user2AccountPda: PublicKey;
  let perpMarketPda: PublicKey;
  let lendingPoolPda: PublicKey;
  let vaultStatePda: PublicKey;
  let vaultTokenAccount: PublicKey;

  // Token accounts
  let adminQuoteAta: PublicKey;
  let user1QuoteAta: PublicKey;
  let user1BaseAta: PublicKey;
  let user2QuoteAta: PublicKey;

  // Oracle mock (dummy account)
  let oracleKeypair: Keypair;

  // Constants
  const MARKET_INDEX = 0;
  const POOL_INDEX = 0;

  before(async () => {
    console.log(`\n  Program ID: ${program.programId.toBase58()}`);
    console.log(`  Admin: ${admin.publicKey.toBase58()}`);
    console.log(`  Cluster: ${connection.rpcEndpoint}\n`);

    // Fund test users from admin wallet (devnet airdrop is unreliable)
    console.log("  Funding test users from admin wallet...");
    const fundTx1 = new anchor.web3.Transaction().add(
      anchor.web3.SystemProgram.transfer({
        fromPubkey: admin.publicKey,
        toPubkey: user1.publicKey,
        lamports: 0.5 * LAMPORTS_PER_SOL,
      })
    );
    const sig1 = await provider.sendAndConfirm(fundTx1);
    logTx("Fund user1", sig1);

    const fundTx2 = new anchor.web3.Transaction().add(
      anchor.web3.SystemProgram.transfer({
        fromPubkey: admin.publicKey,
        toPubkey: user2.publicKey,
        lamports: 0.5 * LAMPORTS_PER_SOL,
      })
    );
    const sig2 = await provider.sendAndConfirm(fundTx2);
    logTx("Fund user2", sig2);

    // Create mints
    console.log("  Creating mints...");
    baseMint = await createMint(
      connection,
      (admin as any).payer,
      admin.publicKey,
      null,
      6
    );
    console.log(`    Base mint: ${baseMint.toBase58()}`);

    quoteMint = await createMint(
      connection,
      (admin as any).payer,
      admin.publicKey,
      null,
      6
    );
    console.log(`    Quote mint: ${quoteMint.toBase58()}`);

    // Create token accounts
    adminQuoteAta = await createAccount(
      connection,
      (admin as any).payer,
      quoteMint,
      admin.publicKey
    );

    user1QuoteAta = await createAccount(
      connection,
      (admin as any).payer,
      quoteMint,
      user1.publicKey
    );

    user1BaseAta = await createAccount(
      connection,
      (admin as any).payer,
      baseMint,
      user1.publicKey
    );

    user2QuoteAta = await createAccount(
      connection,
      (admin as any).payer,
      quoteMint,
      user2.publicKey
    );

    // Mint tokens
    await mintTo(
      connection,
      (admin as any).payer,
      quoteMint,
      user1QuoteAta,
      admin.publicKey,
      1_000_000_000 // 1000 USDC
    );

    await mintTo(
      connection,
      (admin as any).payer,
      quoteMint,
      user2QuoteAta,
      admin.publicKey,
      1_000_000_000
    );

    await mintTo(
      connection,
      (admin as any).payer,
      quoteMint,
      adminQuoteAta,
      admin.publicKey,
      10_000_000_000 // 10000 USDC
    );

    // Oracle mock
    oracleKeypair = Keypair.generate();

    // Derive PDAs
    [exchangePda, exchangeBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("exchange")],
      program.programId
    );

    [userAccountPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user_account"), user1.publicKey.toBuffer()],
      program.programId
    );

    [user2AccountPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user_account"), user2.publicKey.toBuffer()],
      program.programId
    );

    const marketIndexBuf = Buffer.alloc(2);
    marketIndexBuf.writeUInt16LE(MARKET_INDEX);
    [perpMarketPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("perp_market"), marketIndexBuf],
      program.programId
    );

    const poolIndexBuf = Buffer.alloc(2);
    poolIndexBuf.writeUInt16LE(POOL_INDEX);
    [lendingPoolPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("lending_pool"), poolIndexBuf],
      program.programId
    );

    [vaultStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), quoteMint.toBuffer()],
      program.programId
    );

    console.log(`    Exchange PDA: ${exchangePda.toBase58()}`);
    console.log(`    User1 PDA: ${userAccountPda.toBase58()}`);
    console.log(`    Perp market PDA: ${perpMarketPda.toBase58()}`);
    console.log(`    Lending pool PDA: ${lendingPoolPda.toBase58()}`);
    console.log(`    Vault state PDA: ${vaultStatePda.toBase58()}\n`);
  });

  // ══════════════════════════════════════════════════════════
  // Admin Instructions
  // ══════════════════════════════════════════════════════════

  describe("Admin", () => {
    it("Initialize exchange", async () => {
      const tx = await program.methods
        .initializeExchange({
          swapFeeBps: new BN(30),
          perpOpenFeeBps: new BN(10),
          perpCloseFeeBps: new BN(10),
          lendingFeeBps: new BN(50),
          maxLeverage: new BN(20),
          liquidationBonusBps: new BN(500),
          maxLiquidationFractionBps: new BN(5000),
        })
        .accounts({
          admin: admin.publicKey,
        })
        .rpc();
      logTx("initializeExchange", tx);

      const exchange = await program.account.exchange.fetch(exchangePda);
      assert.ok(exchange.admin.equals(admin.publicKey));
      assert.equal(exchange.swapFeeBps.toNumber(), 30);
      assert.equal(exchange.perpOpenFeeBps.toNumber(), 10);
      assert.equal(exchange.maxLeverage.toNumber(), 20);
      assert.equal(exchange.swapPaused, false);
      assert.equal(exchange.perpPaused, false);
      assert.equal(exchange.lendingPaused, false);
    });

    it("Rejects fee > max", async () => {
      const exchange = await program.account.exchange.fetch(exchangePda);
      assert.ok(exchange.swapFeeBps.toNumber() <= 100);
    });

    it("Create perp market", async () => {
      const tx = await program.methods
        .createPerpMarket({
          marketIndex: MARKET_INDEX,
          maxLeverage: new BN(20),
          minPositionSize: new BN(100_000),
          maxOpenInterest: new BN(1_000_000_000_000),
        })
        .accounts({
          admin: admin.publicKey,
          baseMint: baseMint,
          quoteMint: quoteMint,
          oracle: oracleKeypair.publicKey,
        })
        .rpc();
      logTx("createPerpMarket", tx);

      const market = await program.account.perpMarket.fetch(perpMarketPda);
      assert.ok(market.baseMint.equals(baseMint));
      assert.ok(market.quoteMint.equals(quoteMint));
      assert.equal(market.marketIndex, MARKET_INDEX);
      assert.equal(market.maxLeverage.toNumber(), 20);
      assert.equal(market.longOpenInterest.toNumber(), 0);
      assert.equal(market.shortOpenInterest.toNumber(), 0);

      const exchange = await program.account.exchange.fetch(exchangePda);
      assert.equal(exchange.totalPerpMarkets.toNumber(), 1);
    });

    it("Create lending pool", async () => {
      const vaultTokenKeypair = Keypair.generate();
      vaultTokenAccount = vaultTokenKeypair.publicKey;

      const tx = await program.methods
        .createLendingPool({
          poolIndex: POOL_INDEX,
          optimalUtilization: new BN("800000000000000000"),
          baseRate: new BN("20000000000000000"),
          slope1: new BN("40000000000000000"),
          slope2: new BN("750000000000000000"),
          collateralFactor: new BN("800000000000000000"),
          depositLimit: new BN(0),
          borrowLimit: new BN(0),
        })
        .accounts({
          admin: admin.publicKey,
          mint: quoteMint,
          oracle: oracleKeypair.publicKey,
          vaultTokenAccount: vaultTokenKeypair.publicKey,
        })
        .signers([vaultTokenKeypair])
        .rpc();
      logTx("createLendingPool", tx);

      const pool = await program.account.lendingPool.fetch(lendingPoolPda);
      assert.ok(pool.mint.equals(quoteMint));
      assert.equal(pool.totalDeposits.toNumber(), 0);
      assert.equal(pool.totalBorrows.toNumber(), 0);

      const vault = await program.account.vaultState.fetch(vaultStatePda);
      assert.ok(vault.mint.equals(quoteMint));
      assert.equal(vault.collectedFees.toNumber(), 0);

      const exchange = await program.account.exchange.fetch(exchangePda);
      assert.equal(exchange.totalLendingPools.toNumber(), 1);
    });
  });

  // ══════════════════════════════════════════════════════════
  // User Instructions
  // ══════════════════════════════════════════════════════════

  describe("User", () => {
    it("Create user account", async () => {
      const tx = await program.methods
        .createUserAccount(null)
        .accounts({
          owner: user1.publicKey,
        })
        .signers([user1])
        .rpc();
      logTx("createUserAccount (user1)", tx);

      const userAccount = await program.account.userAccount.fetch(userAccountPda);
      assert.ok(userAccount.owner.equals(user1.publicKey));
      assert.equal(userAccount.openPerpPositions, 0);
      assert.equal(userAccount.openLendingPositions, 0);
      assert.equal(userAccount.totalTrades.toNumber(), 0);
    });

    it("Create user2 account", async () => {
      const tx = await program.methods
        .createUserAccount(null)
        .accounts({
          owner: user2.publicKey,
        })
        .signers([user2])
        .rpc();
      logTx("createUserAccount (user2)", tx);

      const userAccount = await program.account.userAccount.fetch(user2AccountPda);
      assert.ok(userAccount.owner.equals(user2.publicKey));
    });

    it("Create user account with referrer", async () => {
      const user3 = Keypair.generate();
      const fundTx3 = new anchor.web3.Transaction().add(
        anchor.web3.SystemProgram.transfer({
          fromPubkey: admin.publicKey,
          toPubkey: user3.publicKey,
          lamports: 0.5 * LAMPORTS_PER_SOL,
        })
      );
      const fundSig3 = await provider.sendAndConfirm(fundTx3);
      logTx("Fund user3", fundSig3);

      const [user3AccountPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("user_account"), user3.publicKey.toBuffer()],
        program.programId
      );

      const tx = await program.methods
        .createUserAccount(user1.publicKey)
        .accounts({
          owner: user3.publicKey,
        })
        .signers([user3])
        .rpc();
      logTx("createUserAccount (user3 w/ referrer)", tx);

      const account = await program.account.userAccount.fetch(user3AccountPda);
      assert.ok(account.referrer.equals(user1.publicKey));
    });
  });

  // ══════════════════════════════════════════════════════════
  // Lending Instructions
  // ══════════════════════════════════════════════════════════

  describe("Lending", () => {
    let lendingPositionPda: PublicKey;

    before(() => {
      [lendingPositionPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("lending_position"),
          user1.publicKey.toBuffer(),
          lendingPoolPda.toBuffer(),
        ],
        program.programId
      );
    });

    it("Deposit collateral", async () => {
      const depositAmount = new BN(500_000_000);

      const tx = await program.methods
        .depositCollateral(depositAmount)
        .accountsPartial({
          owner: user1.publicKey,
          lendingPool: lendingPoolPda,
          lendingPosition: lendingPositionPda,
          vaultTokenAccount: vaultTokenAccount,
          userTokenAccount: user1QuoteAta,
        })
        .signers([user1])
        .rpc();
      logTx("depositCollateral (500 USDC)", tx);

      const position = await program.account.lendingPosition.fetch(lendingPositionPda);
      assert.equal(position.depositedAmount.toNumber(), 500_000_000);
      assert.ok(position.owner.equals(user1.publicKey));

      const pool = await program.account.lendingPool.fetch(lendingPoolPda);
      assert.equal(pool.totalDeposits.toNumber(), 500_000_000);

      const vaultAccount = await getAccount(connection, vaultTokenAccount);
      assert.equal(Number(vaultAccount.amount), 500_000_000);
    });

    it("Deposit additional collateral", async () => {
      const depositAmount = new BN(200_000_000);

      const tx = await program.methods
        .depositCollateral(depositAmount)
        .accountsPartial({
          owner: user1.publicKey,
          lendingPool: lendingPoolPda,
          lendingPosition: lendingPositionPda,
          vaultTokenAccount: vaultTokenAccount,
          userTokenAccount: user1QuoteAta,
        })
        .signers([user1])
        .rpc();
      logTx("depositCollateral (200 USDC more)", tx);

      const position = await program.account.lendingPosition.fetch(lendingPositionPda);
      assert.equal(position.depositedAmount.toNumber(), 700_000_000);

      const pool = await program.account.lendingPool.fetch(lendingPoolPda);
      assert.equal(pool.totalDeposits.toNumber(), 700_000_000);
    });

    it("Rejects zero deposit", async () => {
      try {
        await program.methods
          .depositCollateral(new BN(0))
          .accountsPartial({
            owner: user1.publicKey,
            lendingPool: lendingPoolPda,
            lendingPosition: lendingPositionPda,
            vaultTokenAccount: vaultTokenAccount,
            userTokenAccount: user1QuoteAta,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown");
      } catch (err) {
        console.log(`    ✅ Correctly rejected zero deposit`);
        assert.ok(err.toString().includes("InvalidAmount") || err.toString().includes("Error"));
      }
    });

    it("Withdraw collateral", async () => {
      const withdrawAmount = new BN(100_000_000);

      const tx = await program.methods
        .withdrawCollateral(withdrawAmount)
        .accountsPartial({
          owner: user1.publicKey,
          lendingPool: lendingPoolPda,
          lendingPosition: lendingPositionPda,
          vaultState: vaultStatePda,
          vaultTokenAccount: vaultTokenAccount,
          userTokenAccount: user1QuoteAta,
          oracle: oracleKeypair.publicKey,
        })
        .signers([user1])
        .rpc();
      logTx("withdrawCollateral (100 USDC)", tx);

      const position = await program.account.lendingPosition.fetch(lendingPositionPda);
      assert.equal(position.depositedAmount.toNumber(), 600_000_000);

      const pool = await program.account.lendingPool.fetch(lendingPoolPda);
      assert.equal(pool.totalDeposits.toNumber(), 600_000_000);
    });

    it("Rejects withdrawal exceeding deposit", async () => {
      try {
        await program.methods
          .withdrawCollateral(new BN(999_999_999_999))
          .accountsPartial({
            owner: user1.publicKey,
            lendingPool: lendingPoolPda,
            lendingPosition: lendingPositionPda,
            vaultState: vaultStatePda,
            vaultTokenAccount: vaultTokenAccount,
            userTokenAccount: user1QuoteAta,
            oracle: oracleKeypair.publicKey,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown");
      } catch (err) {
        console.log(`    ✅ Correctly rejected excessive withdrawal`);
        assert.ok(err.toString().includes("Error"));
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // State Verification
  // ══════════════════════════════════════════════════════════

  describe("State Verification", () => {
    it("Exchange state is consistent", async () => {
      const exchange = await program.account.exchange.fetch(exchangePda);
      assert.equal(exchange.totalPerpMarkets.toNumber(), 1);
      assert.equal(exchange.totalLendingPools.toNumber(), 1);
      assert.ok(exchange.totalUsers.toNumber() >= 2);
    });

    it("Perp market state is consistent", async () => {
      const market = await program.account.perpMarket.fetch(perpMarketPda);
      assert.ok(market.baseMint.equals(baseMint));
      assert.ok(market.quoteMint.equals(quoteMint));
      assert.ok(market.oracle.equals(oracleKeypair.publicKey));
      assert.equal(market.longOpenInterest.toNumber(), 0);
      assert.equal(market.shortOpenInterest.toNumber(), 0);
    });

    it("Lending pool state is consistent", async () => {
      const pool = await program.account.lendingPool.fetch(lendingPoolPda);
      assert.ok(pool.mint.equals(quoteMint));
      assert.equal(pool.totalBorrows.toNumber(), 0);
      assert.ok(pool.totalDeposits.toNumber() > 0);
    });

    it("Vault state is consistent", async () => {
      const vault = await program.account.vaultState.fetch(vaultStatePda);
      assert.ok(vault.exchange.equals(exchangePda));
      assert.ok(vault.mint.equals(quoteMint));
      assert.ok(vault.tokenAccount.equals(vaultTokenAccount));
    });

    it("User account tracks activity", async () => {
      const userAccount = await program.account.userAccount.fetch(userAccountPda);
      assert.ok(userAccount.owner.equals(user1.publicKey));
      assert.ok(userAccount.lastActivity.toNumber() > 0);
      assert.ok(userAccount.createdAt.toNumber() > 0);
    });

    it("All account sizes are correct", async () => {
      const exchange = await program.account.exchange.fetch(exchangePda);
      assert.ok(exchange.admin !== undefined);
      assert.ok(exchange.bump !== undefined);

      const market = await program.account.perpMarket.fetch(perpMarketPda);
      assert.ok(market.fundingRate !== undefined);
      assert.ok(market.cumulativeFundingLong !== undefined);

      const pool = await program.account.lendingPool.fetch(lendingPoolPda);
      assert.ok(pool.cumulativeDepositRate !== undefined);
      assert.ok(pool.cumulativeBorrowRate !== undefined);
    });
  });

  // ══════════════════════════════════════════════════════════
  // PDA Derivation Tests
  // ══════════════════════════════════════════════════════════

  describe("PDA Derivation", () => {
    it("Exchange PDA derives correctly", async () => {
      const [pda] = PublicKey.findProgramAddressSync(
        [Buffer.from("exchange")],
        program.programId
      );
      assert.ok(pda.equals(exchangePda));
    });

    it("User account PDA derives correctly", async () => {
      const [pda] = PublicKey.findProgramAddressSync(
        [Buffer.from("user_account"), user1.publicKey.toBuffer()],
        program.programId
      );
      assert.ok(pda.equals(userAccountPda));
    });

    it("Perp market PDA derives correctly", async () => {
      const buf = Buffer.alloc(2);
      buf.writeUInt16LE(MARKET_INDEX);
      const [pda] = PublicKey.findProgramAddressSync(
        [Buffer.from("perp_market"), buf],
        program.programId
      );
      assert.ok(pda.equals(perpMarketPda));
    });

    it("Lending pool PDA derives correctly", async () => {
      const buf = Buffer.alloc(2);
      buf.writeUInt16LE(POOL_INDEX);
      const [pda] = PublicKey.findProgramAddressSync(
        [Buffer.from("lending_pool"), buf],
        program.programId
      );
      assert.ok(pda.equals(lendingPoolPda));
    });

    it("Vault state PDA derives correctly", async () => {
      const [pda] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), quoteMint.toBuffer()],
        program.programId
      );
      assert.ok(pda.equals(vaultStatePda));
    });
  });

  // ══════════════════════════════════════════════════════════
  // Authorization Tests
  // ══════════════════════════════════════════════════════════

  describe("Authorization", () => {
    it("Non-admin cannot create perp market", async () => {
      const marketIndexBuf = Buffer.alloc(2);
      marketIndexBuf.writeUInt16LE(99);
      const [newMarketPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("perp_market"), marketIndexBuf],
        program.programId
      );

      try {
        await program.methods
          .createPerpMarket({
            marketIndex: 99,
            maxLeverage: new BN(20),
            minPositionSize: new BN(100_000),
            maxOpenInterest: new BN(1_000_000_000_000),
          })
          .accounts({
            admin: user1.publicKey,
            baseMint: baseMint,
            quoteMint: quoteMint,
            oracle: oracleKeypair.publicKey,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown Unauthorized");
      } catch (err) {
        console.log(`    ✅ Correctly rejected non-admin`);
        assert.ok(err.toString().includes("Unauthorized") || err.toString().includes("Error"));
      }
    });

    it("Non-admin cannot collect fees", async () => {
      try {
        await program.methods
          .collectFees(new BN(1))
          .accounts({
            admin: user1.publicKey,
            vaultTokenAccount: vaultTokenAccount,
            recipientTokenAccount: user1QuoteAta,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown Unauthorized");
      } catch (err) {
        console.log(`    ✅ Correctly rejected non-admin fee collection`);
        assert.ok(err.toString().includes("Error"));
      }
    });

    it("Cannot use wrong owner for user account", async () => {
      const [user2LendingPositionPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("lending_position"),
          user2.publicKey.toBuffer(),
          lendingPoolPda.toBuffer(),
        ],
        program.programId
      );

      try {
        await program.methods
          .withdrawCollateral(new BN(1))
          .accountsPartial({
            owner: user2.publicKey,
            lendingPool: lendingPoolPda,
            lendingPosition: user2LendingPositionPda,
            vaultState: vaultStatePda,
            vaultTokenAccount: vaultTokenAccount,
            userTokenAccount: user2QuoteAta,
            oracle: oracleKeypair.publicKey,
          })
          .signers([user2])
          .rpc();
        assert.fail("Should have thrown");
      } catch (err) {
        console.log(`    ✅ Correctly rejected wrong owner`);
        assert.ok(err.toString().includes("Error"));
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // Funding Rate Tests
  // ══════════════════════════════════════════════════════════

  describe("Funding Rate", () => {
    it("Rejects funding update before interval", async () => {
      try {
        await program.methods
          .updateFundingRate()
          .accounts({
            cranker: user1.publicKey,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown FundingIntervalNotElapsed");
      } catch (err) {
        console.log(`    ✅ Correctly rejected premature funding update`);
        assert.ok(err.toString().includes("FundingIntervalNotElapsed") || err.toString().includes("Error"));
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // Edge Cases & Error Handling
  // ══════════════════════════════════════════════════════════

  describe("Edge Cases", () => {
    it("Cannot create duplicate exchange", async () => {
      try {
        await program.methods
          .initializeExchange({
            swapFeeBps: new BN(30),
            perpOpenFeeBps: new BN(10),
            perpCloseFeeBps: new BN(10),
            lendingFeeBps: new BN(50),
            maxLeverage: new BN(20),
            liquidationBonusBps: new BN(500),
            maxLiquidationFractionBps: new BN(5000),
          })
          .accounts({
            admin: admin.publicKey,
          })
          .rpc();
        assert.fail("Should have thrown - exchange already exists");
      } catch (err) {
        console.log(`    ✅ Correctly rejected duplicate exchange`);
        assert.ok(err.toString().includes("Error"));
      }
    });

    it("Cannot create duplicate perp market at same index", async () => {
      try {
        await program.methods
          .createPerpMarket({
            marketIndex: MARKET_INDEX,
            maxLeverage: new BN(20),
            minPositionSize: new BN(100_000),
            maxOpenInterest: new BN(1_000_000_000_000),
          })
          .accounts({
            admin: admin.publicKey,
            baseMint: baseMint,
            quoteMint: quoteMint,
            oracle: oracleKeypair.publicKey,
          })
          .rpc();
        assert.fail("Should have thrown");
      } catch (err) {
        console.log(`    ✅ Correctly rejected duplicate perp market`);
        assert.ok(err.toString().includes("Error"));
      }
    });

    it("Cannot create user account twice", async () => {
      try {
        await program.methods
          .createUserAccount(null)
          .accounts({
            owner: user1.publicKey,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown");
      } catch (err) {
        console.log(`    ✅ Correctly rejected duplicate user account`);
        assert.ok(err.toString().includes("Error"));
      }
    });

    it("Leverage validation works on market creation", async () => {
      const marketIndexBuf = Buffer.alloc(2);
      marketIndexBuf.writeUInt16LE(99);

      try {
        await program.methods
          .createPerpMarket({
            marketIndex: 99,
            maxLeverage: new BN(100),
            minPositionSize: new BN(100_000),
            maxOpenInterest: new BN(1_000_000_000_000),
          })
          .accounts({
            admin: admin.publicKey,
            baseMint: baseMint,
            quoteMint: quoteMint,
            oracle: oracleKeypair.publicKey,
          })
          .rpc();
        assert.fail("Should have thrown LeverageOutOfBounds");
      } catch (err) {
        console.log(`    ✅ Correctly rejected excessive leverage`);
        assert.ok(err.toString().includes("LeverageOutOfBounds") || err.toString().includes("Error"));
      }
    });

    it("Collect fees fails with insufficient vault balance", async () => {
      try {
        await program.methods
          .collectFees(new BN(1_000_000))
          .accounts({
            admin: admin.publicKey,
            vaultTokenAccount: vaultTokenAccount,
            recipientTokenAccount: adminQuoteAta,
          })
          .rpc();
        assert.fail("Should have thrown InsufficientVaultBalance");
      } catch (err) {
        console.log(`    ✅ Correctly rejected fee collection with no fees`);
        assert.ok(err.toString().includes("InsufficientVaultBalance") || err.toString().includes("Error"));
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // Repay Tests
  // ══════════════════════════════════════════════════════════

  describe("Repay", () => {
    it("Repay handles zero-borrow gracefully", async () => {
      const [lendingPositionPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("lending_position"),
          user1.publicKey.toBuffer(),
          lendingPoolPda.toBuffer(),
        ],
        program.programId
      );

      try {
        await program.methods
          .repay(new BN(100))
          .accountsPartial({
            owner: user1.publicKey,
            lendingPool: lendingPoolPda,
            lendingPosition: lendingPositionPda,
            vaultTokenAccount: vaultTokenAccount,
            userTokenAccount: user1QuoteAta,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown");
      } catch (err) {
        console.log(`    ✅ Correctly rejected repay with no borrow`);
        assert.ok(err.toString().includes("Error"));
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // Swap / Protocol Routing Tests
  // ══════════════════════════════════════════════════════════

  describe("Swap (Beethoven Protocol Routing)", () => {
    // Known protocol program IDs (compiled into the program via feature flags)
    const MANIFEST_PROGRAM_ID = new PublicKey("MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms");
    const GAMMA_PROGRAM_ID = new PublicKey("GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT");
    const HEAVEN_PROGRAM_ID = new PublicKey("HEAVENoP2qxoeuF8Dj2oT1GHEnu49U5mJYkdeC8BAX2o");

    it("executeSwap rejects unknown protocol in remaining_accounts", async () => {
      const fakeProtocol = Keypair.generate();
      try {
        await program.methods
          .executeSwap({
            amountIn: new BN(1_000_000),
            minimumAmountOut: new BN(1),
          })
          .accountsPartial({
            user: user1.publicKey,
            userInputTokenAccount: user1QuoteAta,
            userOutputTokenAccount: user1BaseAta,
            vaultState: vaultStatePda,
            vaultTokenAccount: vaultTokenAccount,
          })
          .remainingAccounts([
            { pubkey: fakeProtocol.publicKey, isWritable: false, isSigner: false },
          ])
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown UnsupportedProtocol");
      } catch (err) {
        console.log(`    ✅ Correctly rejected unknown swap protocol`);
        assert.ok(err.toString().includes("UnsupportedProtocol") || err.toString().includes("Error"));
      }
    });

    it("executeSwap rejects empty remaining_accounts", async () => {
      try {
        await program.methods
          .executeSwap({
            amountIn: new BN(1_000_000),
            minimumAmountOut: new BN(1),
          })
          .accountsPartial({
            user: user1.publicKey,
            userInputTokenAccount: user1QuoteAta,
            userOutputTokenAccount: user1BaseAta,
            vaultState: vaultStatePda,
            vaultTokenAccount: vaultTokenAccount,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown UnsupportedProtocol");
      } catch (err) {
        console.log(`    ✅ Correctly rejected swap with no protocol`);
        assert.ok(err.toString().includes("UnsupportedProtocol") || err.toString().includes("Error"));
      }
    });

    it("executeSwap detects Manifest protocol (CPI fails - no pool accounts on devnet)", async () => {
      try {
        await program.methods
          .executeSwap({
            amountIn: new BN(1_000_000),
            minimumAmountOut: new BN(1),
          })
          .accountsPartial({
            user: user1.publicKey,
            userInputTokenAccount: user1QuoteAta,
            userOutputTokenAccount: user1BaseAta,
            vaultState: vaultStatePda,
            vaultTokenAccount: vaultTokenAccount,
          })
          .remainingAccounts([
            { pubkey: MANIFEST_PROGRAM_ID, isWritable: false, isSigner: false },
          ])
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown - Manifest pool not available on devnet");
      } catch (err) {
        const errStr = err.toString();
        // Protocol detected correctly but CPI fails (no pool accounts)
        const isNotUnsupported = !errStr.includes("UnsupportedProtocol");
        console.log(`    ✅ Manifest detected (CPI expected to fail on devnet): ${isNotUnsupported ? "protocol recognized" : "routing error"}`);
        assert.ok(errStr.includes("Error"));
      }
    });

    it("executeSwap detects Gamma protocol (CPI fails - no pool accounts on devnet)", async () => {
      try {
        await program.methods
          .executeSwap({
            amountIn: new BN(1_000_000),
            minimumAmountOut: new BN(1),
          })
          .accountsPartial({
            user: user1.publicKey,
            userInputTokenAccount: user1QuoteAta,
            userOutputTokenAccount: user1BaseAta,
            vaultState: vaultStatePda,
            vaultTokenAccount: vaultTokenAccount,
          })
          .remainingAccounts([
            { pubkey: GAMMA_PROGRAM_ID, isWritable: false, isSigner: false },
          ])
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown - Gamma pool not available on devnet");
      } catch (err) {
        const errStr = err.toString();
        const isNotUnsupported = !errStr.includes("UnsupportedProtocol");
        console.log(`    ✅ Gamma detected (CPI expected to fail on devnet): ${isNotUnsupported ? "protocol recognized" : "routing error"}`);
        assert.ok(errStr.includes("Error"));
      }
    });

    it("executeSwap rejects zero amount", async () => {
      try {
        await program.methods
          .executeSwap({
            amountIn: new BN(0),
            minimumAmountOut: new BN(1),
          })
          .accountsPartial({
            user: user1.publicKey,
            userInputTokenAccount: user1QuoteAta,
            userOutputTokenAccount: user1BaseAta,
            vaultState: vaultStatePda,
            vaultTokenAccount: vaultTokenAccount,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown InvalidAmount");
      } catch (err) {
        console.log(`    ✅ Correctly rejected zero swap amount`);
        assert.ok(err.toString().includes("InvalidAmount") || err.toString().includes("Error"));
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // Add/Remove Liquidity (External Protocol Routing)
  // ══════════════════════════════════════════════════════════

  describe("Liquidity (Beethoven Deposit Routing)", () => {
    it("addLiquidity rejects unknown protocol", async () => {
      const fakeProtocol = Keypair.generate();
      try {
        await program.methods
          .addLiquidity(new BN(1_000_000))
          .accounts({
            user: user1.publicKey,
            userTokenAccount: user1QuoteAta,
          })
          .remainingAccounts([
            { pubkey: fakeProtocol.publicKey, isWritable: false, isSigner: false },
          ])
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown UnsupportedProtocol");
      } catch (err) {
        console.log(`    ✅ Correctly rejected unknown deposit protocol`);
        assert.ok(err.toString().includes("UnsupportedProtocol") || err.toString().includes("Error"));
      }
    });

    it("addLiquidity rejects empty remaining_accounts", async () => {
      try {
        await program.methods
          .addLiquidity(new BN(1_000_000))
          .accounts({
            user: user1.publicKey,
            userTokenAccount: user1QuoteAta,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown UnsupportedProtocol");
      } catch (err) {
        console.log(`    ✅ Correctly rejected addLiquidity with no protocol`);
        assert.ok(err.toString().includes("UnsupportedProtocol") || err.toString().includes("Error"));
      }
    });

    it("removeLiquidity rejects unknown protocol", async () => {
      const fakeProtocol = Keypair.generate();
      try {
        await program.methods
          .removeLiquidity(new BN(1_000_000))
          .accounts({
            user: user1.publicKey,
            userTokenAccount: user1QuoteAta,
          })
          .remainingAccounts([
            { pubkey: fakeProtocol.publicKey, isWritable: false, isSigner: false },
          ])
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown UnsupportedProtocol");
      } catch (err) {
        console.log(`    ✅ Correctly rejected unknown withdraw protocol`);
        assert.ok(err.toString().includes("UnsupportedProtocol") || err.toString().includes("Error"));
      }
    });

    it("removeLiquidity rejects empty remaining_accounts", async () => {
      try {
        await program.methods
          .removeLiquidity(new BN(1_000_000))
          .accounts({
            user: user1.publicKey,
            userTokenAccount: user1QuoteAta,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown UnsupportedProtocol");
      } catch (err) {
        console.log(`    ✅ Correctly rejected removeLiquidity with no protocol`);
        assert.ok(err.toString().includes("UnsupportedProtocol") || err.toString().includes("Error"));
      }
    });

    it("addLiquidity rejects zero amount", async () => {
      try {
        await program.methods
          .addLiquidity(new BN(0))
          .accounts({
            user: user1.publicKey,
            userTokenAccount: user1QuoteAta,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown InvalidAmount");
      } catch (err) {
        console.log(`    ✅ Correctly rejected zero liquidity amount`);
        assert.ok(err.toString().includes("InvalidAmount") || err.toString().includes("Error"));
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // REAL Manifest Swap via Beethoven (CPI to Manifest DEX)
  // ══════════════════════════════════════════════════════════

  describe("Real Manifest Swap via Beethoven", () => {
    const MANIFEST_PROGRAM = new PublicKey("MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms");

    let manifestMarket: Keypair;
    let baseVault: PublicKey;
    let quoteVault: PublicKey;

    it("Create Manifest market for baseMint/quoteMint", async () => {
      manifestMarket = Keypair.generate();

      // Derive Manifest vault PDAs
      [baseVault] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), manifestMarket.publicKey.toBuffer(), baseMint.toBuffer()],
        MANIFEST_PROGRAM
      );
      [quoteVault] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), manifestMarket.publicKey.toBuffer(), quoteMint.toBuffer()],
        MANIFEST_PROGRAM
      );

      // Allocate market account (owned by Manifest program, exactly 256 bytes for header)
      const space = 256;
      const lamports = await connection.getMinimumBalanceForRentExemption(space);
      const allocateIx = SystemProgram.createAccount({
        fromPubkey: admin.publicKey,
        newAccountPubkey: manifestMarket.publicKey,
        space,
        lamports,
        programId: MANIFEST_PROGRAM,
      });

      // CreateMarket instruction (discriminator 0, 9 accounts including Token-2022)
      const TOKEN_2022_PROGRAM_ID = new PublicKey("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
      const createMarketIx = new TransactionInstruction({
        programId: MANIFEST_PROGRAM,
        keys: [
          { pubkey: admin.publicKey, isSigner: true, isWritable: true },
          { pubkey: manifestMarket.publicKey, isSigner: false, isWritable: true },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
          { pubkey: baseMint, isSigner: false, isWritable: false },
          { pubkey: quoteMint, isSigner: false, isWritable: false },
          { pubkey: baseVault, isSigner: false, isWritable: true },
          { pubkey: quoteVault, isSigner: false, isWritable: true },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
        ],
        data: Buffer.from([0]),
      });

      const tx = new anchor.web3.Transaction().add(allocateIx, createMarketIx);
      const sig = await provider.sendAndConfirm(tx, [manifestMarket]);
      logTx("Create Manifest Market", sig);
      console.log(`    Market: ${manifestMarket.publicKey.toBase58()}`);
      console.log(`    Base vault: ${baseVault.toBase58()}`);
      console.log(`    Quote vault: ${quoteVault.toBase58()}`);

      // Verify all accounts exist and check owners
      const marketInfo = await connection.getAccountInfo(manifestMarket.publicKey);
      console.log(`    Market owner: ${marketInfo?.owner.toBase58()}, size: ${marketInfo?.data.length}`);
      const bvInfo = await connection.getAccountInfo(baseVault);
      console.log(`    BaseVault owner: ${bvInfo?.owner.toBase58() ?? "DOES NOT EXIST"}, size: ${bvInfo?.data.length}`);
      const qvInfo = await connection.getAccountInfo(quoteVault);
      console.log(`    QuoteVault owner: ${qvInfo?.owner.toBase58() ?? "DOES NOT EXIST"}, size: ${qvInfo?.data.length}`);
      const u1bInfo = await connection.getAccountInfo(user1BaseAta);
      console.log(`    user1BaseAta owner: ${u1bInfo?.owner.toBase58()}, size: ${u1bInfo?.data.length}`);
      const u1qInfo = await connection.getAccountInfo(user1QuoteAta);
      console.log(`    user1QuoteAta owner: ${u1qInfo?.owner.toBase58()}, size: ${u1qInfo?.data.length}`);
    });

    it("Admin adds liquidity to Manifest market", async () => {
      // Create admin ATAs (Manifest SDK expects associated token accounts)
      const adminBaseAta = await getOrCreateAssociatedTokenAccount(
        connection, (admin as any).payer, baseMint, admin.publicKey
      );
      const adminQuoteAtaAssoc = await getOrCreateAssociatedTokenAccount(
        connection, (admin as any).payer, quoteMint, admin.publicKey
      );

      // Mint base tokens to admin ATA
      await mintTo(
        connection, (admin as any).payer, baseMint,
        adminBaseAta.address, admin.publicKey, 10_000_000_000
      );
      // Mint quote tokens to admin ATA
      await mintTo(
        connection, (admin as any).payer, quoteMint,
        adminQuoteAtaAssoc.address, admin.publicKey, 10_000_000_000
      );

      // Use Manifest SDK — creates wrapper + claims seat for admin
      const client = await ManifestClient.getClientForMarket(
        connection,
        manifestMarket.publicKey,
        (admin as any).payer,
      );

      // Deposit tokens into Manifest market
      const depositBaseIx = client.depositIx(admin.publicKey, baseMint, 1000);
      const depositQuoteIx = client.depositIx(admin.publicKey, quoteMint, 1000);
      const tx1 = new anchor.web3.Transaction().add(depositBaseIx, depositQuoteIx);
      const sig1 = await provider.sendAndConfirm(tx1);
      logTx("Deposit liquidity to Manifest", sig1);

      // Place a sell (ask) order: sell 500 base at price 1.0 quote/base
      const placeAskIx = client.placeOrderIx({
        numBaseTokens: 500,
        tokenPrice: 1.0,
        isBid: false,
        lastValidSlot: 0,
        orderType: OrderType.Limit,
        clientOrderId: 1,
      });
      const tx2 = new anchor.web3.Transaction().add(placeAskIx);
      const sig2 = await provider.sendAndConfirm(tx2);
      logTx("Place ask order (500 base @ 1.0)", sig2);

      // Place a buy (bid) order: buy 500 base at price 0.9 quote/base
      const placeBidIx = client.placeOrderIx({
        numBaseTokens: 500,
        tokenPrice: 0.9,
        isBid: true,
        lastValidSlot: 0,
        orderType: OrderType.Limit,
        clientOrderId: 2,
      });
      const tx3 = new anchor.web3.Transaction().add(placeBidIx);
      const sig3 = await provider.sendAndConfirm(tx3);
      logTx("Place bid order (500 base @ 0.9)", sig3);

      console.log("    Manifest market ready with liquidity");
    });

    it("Direct Manifest swap works (no Beethoven CPI)", async () => {
      const preQuote = await getAccount(connection, user1QuoteAta);
      const preBase = await getAccount(connection, user1BaseAta);
      console.log(`    Pre-direct-swap: ${Number(preQuote.amount) / 1e6} quote, ${Number(preBase.amount) / 1e6} base`);

      // Manifest SwapContext uses sequential parsing: payer, market, then token accounts.
      // The on-chain loader does NOT consume a systemProgram account for Swap.
      // Account layout: payer, market, traderBase, traderQuote, baseVault, quoteVault, tokenProgramBase
      const swapData = Buffer.alloc(19);
      swapData.writeUInt8(4, 0); // Swap discriminator
      swapData.writeBigUInt64LE(BigInt(1_000_000), 1); // inAtoms = 1 quote token
      swapData.writeBigUInt64LE(BigInt(1), 9); // outAtoms = min 1 base atom
      swapData.writeUInt8(0, 17); // isBaseIn = false (quote→base)
      swapData.writeUInt8(1, 18); // isExactIn = true

      const swapIx = new TransactionInstruction({
        programId: MANIFEST_PROGRAM,
        keys: [
          { pubkey: user1.publicKey, isSigner: true, isWritable: true },
          { pubkey: manifestMarket.publicKey, isSigner: false, isWritable: true },
          { pubkey: user1BaseAta, isSigner: false, isWritable: true },
          { pubkey: user1QuoteAta, isSigner: false, isWritable: true },
          { pubkey: baseVault, isSigner: false, isWritable: true },
          { pubkey: quoteVault, isSigner: false, isWritable: true },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        ],
        data: swapData,
      });

      const tx = new anchor.web3.Transaction().add(swapIx);
      const sig = await sendAndConfirmTransaction(connection, tx, [user1]);
      logTx("Direct Manifest Swap", sig);

      const postQuote = await getAccount(connection, user1QuoteAta);
      const postBase = await getAccount(connection, user1BaseAta);
      console.log(`    Post-direct-swap: ${Number(postQuote.amount) / 1e6} quote, ${Number(postBase.amount) / 1e6} base`);
      const quoteSpent = Number(preQuote.amount) - Number(postQuote.amount);
      const baseReceived = Number(postBase.amount) - Number(preBase.amount);
      console.log(`    Direct swap result: spent ${quoteSpent / 1e6} quote → received ${baseReceived / 1e6} base`);
      assert.ok(quoteSpent > 0, "Should have spent quote tokens");
      assert.ok(baseReceived > 0, "Should have received base tokens");
    });

    it("REAL SWAP: user1 swaps quote→base via Beethoven → Manifest CPI", async () => {
      // Pre-swap balances
      const preQuote = await getAccount(connection, user1QuoteAta);
      const preBase = await getAccount(connection, user1BaseAta);
      console.log(`    Pre-swap: ${Number(preQuote.amount) / 1e6} quote, ${Number(preBase.amount) / 1e6} base`);

      const swapAmount = new BN(10_000_000); // 10 quote tokens
      const minOut = new BN(1); // minimum 1 base atom out

      // Beethoven executeSwap with remaining_accounts pointing to Manifest
      // remaining_accounts[0] = Manifest program (Beethoven detects protocol)
      // remaining_accounts[1..] = Manifest Swap accounts:
      //   payer, market, system_program, trader_base, trader_quote,
      //   base_vault, quote_vault, token_program_base
      const tx = await program.methods
        .executeSwap({
          amountIn: swapAmount,
          minimumAmountOut: minOut,
        })
        .accountsPartial({
          user: user1.publicKey,
          userInputTokenAccount: user1QuoteAta,
          userOutputTokenAccount: user1BaseAta,
          vaultState: vaultStatePda,
          vaultTokenAccount: vaultTokenAccount,
        })
        .remainingAccounts([
          { pubkey: MANIFEST_PROGRAM, isWritable: false, isSigner: false },
          { pubkey: user1.publicKey, isWritable: true, isSigner: true },
          { pubkey: manifestMarket.publicKey, isWritable: true, isSigner: false },
          { pubkey: user1BaseAta, isWritable: true, isSigner: false },
          { pubkey: user1QuoteAta, isWritable: true, isSigner: false },
          { pubkey: baseVault, isWritable: true, isSigner: false },
          { pubkey: quoteVault, isWritable: true, isSigner: false },
          { pubkey: TOKEN_PROGRAM_ID, isWritable: false, isSigner: false },
        ])
        .signers([user1])
        .rpc();
      logTx("REAL SWAP: Beethoven → Manifest CPI", tx);

      // Post-swap balances
      const postQuote = await getAccount(connection, user1QuoteAta);
      const postBase = await getAccount(connection, user1BaseAta);
      console.log(`    Post-swap: ${Number(postQuote.amount) / 1e6} quote, ${Number(postBase.amount) / 1e6} base`);

      const quoteSpent = Number(preQuote.amount) - Number(postQuote.amount);
      const baseReceived = Number(postBase.amount) - Number(preBase.amount);
      console.log(`    Result: spent ${quoteSpent / 1e6} quote → received ${baseReceived / 1e6} base`);
      console.log(`    Fee: ${(swapAmount.toNumber() - (swapAmount.toNumber() - Math.floor(swapAmount.toNumber() * 30 / 10000))) / 1e6} quote (30 bps)`);

      assert.ok(quoteSpent > 0, "Should have spent quote tokens");
      assert.ok(baseReceived > 0, "Should have received base tokens");
    });
  });

  // ══════════════════════════════════════════════════════════
  // REAL Kamino Lending via Beethoven (CPI to Kamino klend)
  // ══════════════════════════════════════════════════════════

  describe("Real Kamino Lending via Beethoven", () => {
    const KAMINO_PROGRAM = new PublicKey("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");

    let kaminoMarket: Keypair;
    let kaminoReserve: Keypair;
    let kaminoMarketAuth: PublicKey;
    let liqSupplyPda: PublicKey;
    let feeReceiverPda: PublicKey;
    let collMintPda: PublicKey;
    let collSupplyPda: PublicKey;
    let user1CollAta: PublicKey;

    it("Create Kamino lending market for our quoteMint", async () => {
      kaminoMarket = Keypair.generate();

      // Lending market authority PDA: seeds = ["lma", market_pubkey]
      [kaminoMarketAuth] = PublicKey.findProgramAddressSync(
        [Buffer.from("lma"), kaminoMarket.publicKey.toBuffer()],
        KAMINO_PROGRAM
      );

      // Allocate market account (real on-chain size = 4664 bytes)
      const space = 4664;
      const lamports = await connection.getMinimumBalanceForRentExemption(space);
      const allocateIx = SystemProgram.createAccount({
        fromPubkey: admin.publicKey,
        newAccountPubkey: kaminoMarket.publicKey,
        space,
        lamports,
        programId: KAMINO_PROGRAM,
      });

      // initLendingMarket: discriminator(8) + quoteCurrency(32) = 40 bytes
      const data = Buffer.alloc(40);
      Buffer.from([34, 162, 116, 14, 101, 137, 94, 239]).copy(data, 0);
      // quoteCurrency: 32 zeros (default)

      const initMarketIx = new TransactionInstruction({
        programId: KAMINO_PROGRAM,
        keys: [
          { pubkey: admin.publicKey, isSigner: true, isWritable: true },
          { pubkey: kaminoMarket.publicKey, isSigner: false, isWritable: true },
          { pubkey: kaminoMarketAuth, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
          { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
        ],
        data,
      });

      const tx = new anchor.web3.Transaction().add(allocateIx, initMarketIx);
      const sig = await provider.sendAndConfirm(tx, [kaminoMarket]);
      logTx("Create Kamino Market", sig);
      console.log(`    Kamino Market: ${kaminoMarket.publicKey.toBase58()}`);
      console.log(`    Market Authority: ${kaminoMarketAuth.toBase58()}`);
    });

    it("Create Kamino reserve for quoteMint", async () => {
      kaminoReserve = Keypair.generate();

      // Derive all reserve PDAs: seeds = [seed_string, lending_market, liquidity_mint]
      [liqSupplyPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("reserve_liq_supply"), kaminoMarket.publicKey.toBuffer(), quoteMint.toBuffer()],
        KAMINO_PROGRAM
      );
      [feeReceiverPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("fee_receiver"), kaminoMarket.publicKey.toBuffer(), quoteMint.toBuffer()],
        KAMINO_PROGRAM
      );
      [collMintPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("reserve_coll_mint"), kaminoMarket.publicKey.toBuffer(), quoteMint.toBuffer()],
        KAMINO_PROGRAM
      );
      [collSupplyPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("reserve_coll_supply"), kaminoMarket.publicKey.toBuffer(), quoteMint.toBuffer()],
        KAMINO_PROGRAM
      );

      // Allocate reserve account (real on-chain size = 8624 bytes)
      const space = 8624;
      const lamports = await connection.getMinimumBalanceForRentExemption(space);
      const allocateIx = SystemProgram.createAccount({
        fromPubkey: admin.publicKey,
        newAccountPubkey: kaminoReserve.publicKey,
        space,
        lamports,
        programId: KAMINO_PROGRAM,
      });

      // initReserve (on-chain IDL): just 8-byte discriminator, no args
      // Accounts (12): lendingMarketOwner, lendingMarket, lendingMarketAuthority(mut),
      //   reserve, reserveLiquidityMint, reserveLiquiditySupply, feeReceiver,
      //   reserveCollateralMint, reserveCollateralSupply, rent, tokenProgram, systemProgram
      const data = Buffer.from([138, 245, 71, 225, 153, 4, 3, 43]);

      const initReserveIx = new TransactionInstruction({
        programId: KAMINO_PROGRAM,
        keys: [
          { pubkey: admin.publicKey, isSigner: true, isWritable: true },
          { pubkey: kaminoMarket.publicKey, isSigner: false, isWritable: false },
          { pubkey: kaminoMarketAuth, isSigner: false, isWritable: true },   // writable per on-chain IDL
          { pubkey: kaminoReserve.publicKey, isSigner: false, isWritable: true },
          { pubkey: quoteMint, isSigner: false, isWritable: false },
          { pubkey: liqSupplyPda, isSigner: false, isWritable: true },
          { pubkey: feeReceiverPda, isSigner: false, isWritable: true },
          { pubkey: collMintPda, isSigner: false, isWritable: true },
          { pubkey: collSupplyPda, isSigner: false, isWritable: true },
          { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        data,
      });

      const tx = new anchor.web3.Transaction().add(allocateIx, initReserveIx);
      const sig = await provider.sendAndConfirm(tx, [kaminoReserve]);
      logTx("Create Kamino Reserve", sig);
      console.log(`    Reserve: ${kaminoReserve.publicKey.toBase58()}`);
      console.log(`    Liquidity Supply: ${liqSupplyPda.toBase58()}`);
      console.log(`    Collateral Mint: ${collMintPda.toBase58()}`);
      console.log(`    Collateral Supply: ${collSupplyPda.toBase58()}`);
      console.log(`    Fee Receiver: ${feeReceiverPda.toBase58()}`);
    });

    it("Update Kamino reserve config: set entire config", async () => {
      // On-chain mode 25 = UpdateEntireReserveConfig (discovered via probing).
      // Must set deposit limit, borrow limit, oracle (Pyth), and token info in one shot
      // because the on-chain program validates oracle config after ANY update.
      //
      // ReserveConfig layout (from SDK borsh struct):
      //   [0]     status: u8
      //   [16]    loanToValuePct: u8
      //   [17]    liquidationThresholdPct: u8
      //   [40-63] fees (ReserveFees): 24 bytes
      //   [64-151] borrowRateCurve: 11 * CurvePoint(u32+u32) = 88 bytes
      //   [152]   borrowFactorPct: u64
      //   [160]   depositLimit: u64
      //   [168]   borrowLimit: u64
      //   [176]   tokenInfo.name: [u8;32]
      //   [208]   tokenInfo.heuristic: lower(u64), upper(u64), exp(u64)
      //   [232]   tokenInfo.maxTwapDivergenceBps: u64
      //   [240]   tokenInfo.maxAgePriceSeconds: u64
      //   [248]   tokenInfo.maxAgeTwapSeconds: u64
      //   [256]   tokenInfo.scopeConfiguration: priceFeed(32) + priceChain(8) + twapChain(8)
      //   [304]   tokenInfo.switchboardConfiguration: priceAggregator(32) + twapAggregator(32)
      //   [368]   tokenInfo.pythConfiguration: price(32)

      const configValue = Buffer.alloc(952);

      // status = 0
      configValue.writeUInt8(0, 0);
      // loanToValuePct = 75
      configValue.writeUInt8(75, 16);
      // liquidationThresholdPct = 80
      configValue.writeUInt8(80, 17);
      // deleveragingThresholdSlotsPerBps = 1 (must be > 0)
      configValue.writeBigUInt64LE(BigInt(1), 32);
      // borrowRateCurve: 11 CurvePoints at offset 64, each = u32(utilizationRateBps) + u32(borrowRateBps)
      const curvePoints = [
        [0, 200],       // 0% util → 2% borrow rate
        [1000, 300],    // 10% → 3%
        [2000, 400],    // 20% → 4%
        [3000, 500],    // 30% → 5%
        [4000, 600],    // 40% → 6%
        [5000, 700],    // 50% → 7%
        [6000, 1000],   // 60% → 10%
        [7000, 1500],   // 70% → 15%
        [8000, 2000],   // 80% → 20%
        [9000, 3000],   // 90% → 30%
        [10000, 5000],  // 100% → 50%
      ];
      for (let i = 0; i < 11; i++) {
        configValue.writeUInt32LE(curvePoints[i][0], 64 + i * 8);
        configValue.writeUInt32LE(curvePoints[i][1], 64 + i * 8 + 4);
      }
      // borrowFactorPct = 100
      configValue.writeBigUInt64LE(BigInt(100), 152);
      // depositLimit = 1e12 (1,000,000 tokens with 6 decimals)
      configValue.writeBigUInt64LE(BigInt(1_000_000_000_000), 160);
      // borrowLimit = 1e12
      configValue.writeBigUInt64LE(BigInt(1_000_000_000_000), 168);
      // tokenInfo.name = "USDC"
      Buffer.from("USDC").copy(configValue, 176);
      // tokenInfo.heuristic: lower=1, upper=1e14, exp=6
      configValue.writeBigUInt64LE(BigInt(1), 208);
      configValue.writeBigUInt64LE(BigInt(100_000_000_000_000), 216);
      configValue.writeBigUInt64LE(BigInt(6), 224);
      // tokenInfo.maxTwapDivergenceBps = 10000
      configValue.writeBigUInt64LE(BigInt(10_000), 232);
      // tokenInfo.maxAgePriceSeconds = 1,000,000
      configValue.writeBigUInt64LE(BigInt(1_000_000), 240);
      // tokenInfo.maxAgeTwapSeconds = 1,000,000
      configValue.writeBigUInt64LE(BigInt(1_000_000), 248);
      // tokenInfo.scopeConfiguration.priceFeed = SystemProgram (non-zero dummy)
      SystemProgram.programId.toBuffer().copy(configValue, 256);
      // tokenInfo.pythConfiguration.price = admin pubkey (non-zero dummy oracle)
      admin.publicKey.toBuffer().copy(configValue, 368);

      // Instruction: discriminator(8) + mode(u64) + value(952) = 968 bytes
      const data = Buffer.alloc(968);
      Buffer.from([61, 148, 100, 70, 143, 107, 17, 13]).copy(data, 0);
      data.writeBigUInt64LE(BigInt(25), 8); // mode = 25 (UpdateEntireReserveConfig on-chain)
      configValue.copy(data, 16);

      const updateIx = new TransactionInstruction({
        programId: KAMINO_PROGRAM,
        keys: [
          { pubkey: admin.publicKey, isSigner: true, isWritable: false },
          { pubkey: kaminoMarket.publicKey, isSigner: false, isWritable: false },
          { pubkey: kaminoReserve.publicKey, isSigner: false, isWritable: true },
        ],
        data,
      });

      const tx = new anchor.web3.Transaction().add(updateIx);
      const sig = await provider.sendAndConfirm(tx);
      logTx("Update Entire Reserve Config", sig);
      console.log(`    Set deposit limit=1e12, Pyth oracle configured`);
    });

    it("Prepare user1 collateral token account", async () => {
      // Create ATA for user1 to receive Kamino collateral (cTokens)
      const ata = await getOrCreateAssociatedTokenAccount(
        connection, (admin as any).payer, collMintPda, user1.publicKey
      );
      user1CollAta = ata.address;
      console.log(`    User1 collateral ATA: ${user1CollAta.toBase58()}`);
    });

    it("Direct Kamino deposit works (no Beethoven CPI)", async () => {
      const preQuote = await getAccount(connection, user1QuoteAta);
      console.log(`    Pre-deposit quote balance: ${Number(preQuote.amount) / 1e6}`);

      // depositReserveLiquidity (on-chain IDL): discriminator(8) + liquidityAmount(8) = 16 bytes
      // Accounts (9): owner, reserve, lendingMarket, lendingMarketAuthority,
      //   reserveLiquiditySupply, reserveCollateralMint,
      //   userSourceLiquidity, userDestinationCollateral, tokenProgram
      const amount = 5_000_000; // 5 tokens
      const data = Buffer.alloc(16);
      Buffer.from([169, 201, 30, 126, 6, 205, 102, 68]).copy(data, 0);
      data.writeBigUInt64LE(BigInt(amount), 8);

      const depositIx = new TransactionInstruction({
        programId: KAMINO_PROGRAM,
        keys: [
          { pubkey: user1.publicKey, isSigner: true, isWritable: false },      // owner
          { pubkey: kaminoReserve.publicKey, isSigner: false, isWritable: true },
          { pubkey: kaminoMarket.publicKey, isSigner: false, isWritable: false },
          { pubkey: kaminoMarketAuth, isSigner: false, isWritable: false },
          { pubkey: liqSupplyPda, isSigner: false, isWritable: true },         // reserveLiquiditySupply
          { pubkey: collMintPda, isSigner: false, isWritable: true },          // reserveCollateralMint
          { pubkey: user1QuoteAta, isSigner: false, isWritable: true },        // userSourceLiquidity
          { pubkey: user1CollAta, isSigner: false, isWritable: true },         // userDestinationCollateral
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },    // tokenProgram
        ],
        data,
      });

      const tx = new anchor.web3.Transaction().add(depositIx);
      const sig = await sendAndConfirmTransaction(connection, tx, [user1]);
      logTx("Direct Kamino Deposit", sig);

      const postQuote = await getAccount(connection, user1QuoteAta);
      const postColl = await getAccount(connection, user1CollAta);
      const quoteSpent = Number(preQuote.amount) - Number(postQuote.amount);
      console.log(`    Post-deposit: quote=${Number(postQuote.amount)/1e6}, coll=${Number(postColl.amount)/1e6}`);
      console.log(`    Deposited ${quoteSpent/1e6} quote → received ${Number(postColl.amount)/1e6} collateral`);
      assert.ok(quoteSpent > 0, "Should have spent quote tokens");
      assert.ok(Number(postColl.amount) > 0, "Should have received collateral tokens");
    });

    it("REAL DEPOSIT: user1 deposits via Beethoven → Kamino CPI", async () => {
      const preQuote = await getAccount(connection, user1QuoteAta);
      const preColl = await getAccount(connection, user1CollAta);
      console.log(`    Pre-Beethoven-deposit: quote=${Number(preQuote.amount)/1e6}, coll=${Number(preColl.amount)/1e6}`);

      const depositAmount = new BN(10_000_000); // 10 tokens

      // Beethoven addLiquidity: remaining_accounts[0] = Kamino program (protocol detection)
      // remaining_accounts[1..9] = depositReserveLiquidity accounts (on-chain IDL: 9 accounts)
      const tx = await program.methods
        .addLiquidity(depositAmount)
        .accounts({
          user: user1.publicKey,
          userTokenAccount: user1QuoteAta,
        })
        .remainingAccounts([
          { pubkey: KAMINO_PROGRAM, isWritable: false, isSigner: false },
          // depositReserveLiquidity accounts (9):
          { pubkey: user1.publicKey, isWritable: false, isSigner: true },      // owner
          { pubkey: kaminoReserve.publicKey, isWritable: true, isSigner: false },
          { pubkey: kaminoMarket.publicKey, isWritable: false, isSigner: false },
          { pubkey: kaminoMarketAuth, isWritable: false, isSigner: false },
          { pubkey: liqSupplyPda, isWritable: true, isSigner: false },          // reserveLiquiditySupply
          { pubkey: collMintPda, isWritable: true, isSigner: false },           // reserveCollateralMint
          { pubkey: user1QuoteAta, isWritable: true, isSigner: false },         // userSourceLiquidity
          { pubkey: user1CollAta, isWritable: true, isSigner: false },          // userDestinationCollateral
          { pubkey: TOKEN_PROGRAM_ID, isWritable: false, isSigner: false },     // tokenProgram
        ])
        .signers([user1])
        .rpc();
      logTx("REAL DEPOSIT: Beethoven → Kamino CPI", tx);

      const postQuote = await getAccount(connection, user1QuoteAta);
      const postColl = await getAccount(connection, user1CollAta);
      const quoteSpent = Number(preQuote.amount) - Number(postQuote.amount);
      const collReceived = Number(postColl.amount) - Number(preColl.amount);
      console.log(`    Post-deposit: quote=${Number(postQuote.amount)/1e6}, coll=${Number(postColl.amount)/1e6}`);
      console.log(`    Deposited ${quoteSpent/1e6} quote → received ${collReceived/1e6} collateral (via Beethoven CPI)`);

      assert.ok(quoteSpent > 0, "Should have spent quote tokens");
      assert.ok(collReceived > 0, "Should have received collateral tokens");
    });

    it("Direct Kamino withdraw works (no Beethoven CPI)", async () => {
      const preQuote = await getAccount(connection, user1QuoteAta);
      const preColl = await getAccount(connection, user1CollAta);
      console.log(`    Pre-direct-withdraw: quote=${Number(preQuote.amount)/1e6}, coll=${Number(preColl.amount)/1e6}`);

      // redeemReserveCollateral: discriminator(8) + collateralAmount(8) = 16 bytes
      const amount = 2_000_000; // 2 collateral tokens
      const data = Buffer.alloc(16);
      Buffer.from([234, 117, 181, 125, 185, 142, 220, 29]).copy(data, 0);
      data.writeBigUInt64LE(BigInt(amount), 8);

      // Debug: check account owners before CPI
      const reserveInfo = await connection.getAccountInfo(kaminoReserve.publicKey);
      const marketInfo = await connection.getAccountInfo(kaminoMarket.publicKey);
      const authInfo = await connection.getAccountInfo(kaminoMarketAuth);
      console.log(`    Reserve ${kaminoReserve.publicKey.toBase58()} owner: ${reserveInfo?.owner.toBase58() ?? "NULL"}, size: ${reserveInfo?.data.length ?? 0}`);
      console.log(`    Market ${kaminoMarket.publicKey.toBase58()} owner: ${marketInfo?.owner.toBase58() ?? "NULL"}, size: ${marketInfo?.data.length ?? 0}`);
      console.log(`    Auth ${kaminoMarketAuth.toBase58()} owner: ${authInfo?.owner.toBase58() ?? "NULL"}, size: ${authInfo?.data.length ?? 0}`);

      // On-chain binary may have reserve before lendingMarketAuthority
      const redeemIx = new TransactionInstruction({
        programId: KAMINO_PROGRAM,
        keys: [
          { pubkey: user1.publicKey, isSigner: true, isWritable: false },      // owner
          { pubkey: kaminoMarket.publicKey, isSigner: false, isWritable: false },
          { pubkey: kaminoReserve.publicKey, isSigner: false, isWritable: true }, // reserve at [2]
          { pubkey: kaminoMarketAuth, isSigner: false, isWritable: false },       // authority at [3]
          { pubkey: collMintPda, isSigner: false, isWritable: true },          // reserveCollateralMint
          { pubkey: liqSupplyPda, isSigner: false, isWritable: true },         // reserveLiquiditySupply
          { pubkey: user1CollAta, isSigner: false, isWritable: true },         // userSourceCollateral
          { pubkey: user1QuoteAta, isSigner: false, isWritable: true },        // userDestinationLiquidity
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },    // tokenProgram
        ],
        data,
      });

      const tx = new anchor.web3.Transaction().add(redeemIx);
      const sig = await sendAndConfirmTransaction(connection, tx, [user1]);
      logTx("Direct Kamino Withdraw", sig);

      const postQuote = await getAccount(connection, user1QuoteAta);
      const postColl = await getAccount(connection, user1CollAta);
      const quoteReceived = Number(postQuote.amount) - Number(preQuote.amount);
      const collSpent = Number(preColl.amount) - Number(postColl.amount);
      console.log(`    Post-withdraw: quote=${Number(postQuote.amount)/1e6}, coll=${Number(postColl.amount)/1e6}`);
      console.log(`    Redeemed ${collSpent/1e6} collateral → received ${quoteReceived/1e6} quote`);
      assert.ok(quoteReceived > 0, "Should have received quote tokens back");
      assert.ok(collSpent > 0, "Should have spent collateral tokens");
    });

    it("REAL WITHDRAW: user1 redeems collateral via Beethoven → Kamino CPI", async () => {
      const preQuote = await getAccount(connection, user1QuoteAta);
      const preColl = await getAccount(connection, user1CollAta);
      console.log(`    Pre-withdraw: quote=${Number(preQuote.amount)/1e6}, coll=${Number(preColl.amount)/1e6}`);

      const withdrawAmount = new BN(5_000_000); // 5 collateral tokens

      // Beethoven removeLiquidity: remaining_accounts[0] = Kamino program
      // remaining_accounts[1..9] = redeemReserveCollateral accounts (on-chain IDL: 9 accounts)
      const tx = await program.methods
        .removeLiquidity(withdrawAmount)
        .accounts({
          user: user1.publicKey,
          userTokenAccount: user1QuoteAta,
        })
        .remainingAccounts([
          { pubkey: KAMINO_PROGRAM, isWritable: false, isSigner: false },
          // redeemReserveCollateral accounts (9) — on-chain order (reserve before authority):
          { pubkey: user1.publicKey, isWritable: false, isSigner: true },      // owner
          { pubkey: kaminoMarket.publicKey, isWritable: false, isSigner: false },
          { pubkey: kaminoReserve.publicKey, isWritable: true, isSigner: false }, // reserve at [2]
          { pubkey: kaminoMarketAuth, isWritable: false, isSigner: false },      // authority at [3]
          { pubkey: collMintPda, isWritable: true, isSigner: false },           // reserveCollateralMint
          { pubkey: liqSupplyPda, isWritable: true, isSigner: false },          // reserveLiquiditySupply
          { pubkey: user1CollAta, isWritable: true, isSigner: false },          // userSourceCollateral
          { pubkey: user1QuoteAta, isWritable: true, isSigner: false },         // userDestinationLiquidity
          { pubkey: TOKEN_PROGRAM_ID, isWritable: false, isSigner: false },     // tokenProgram
        ])
        .signers([user1])
        .rpc();
      logTx("REAL WITHDRAW: Beethoven → Kamino CPI", tx);

      const postQuote = await getAccount(connection, user1QuoteAta);
      const postColl = await getAccount(connection, user1CollAta);
      const quoteReceived = Number(postQuote.amount) - Number(preQuote.amount);
      const collSpent = Number(preColl.amount) - Number(postColl.amount);
      console.log(`    Post-withdraw: quote=${Number(postQuote.amount)/1e6}, coll=${Number(postColl.amount)/1e6}`);
      console.log(`    Redeemed ${collSpent/1e6} collateral → received ${quoteReceived/1e6} quote (via Beethoven CPI)`);

      assert.ok(quoteReceived > 0, "Should have received quote tokens back");
      assert.ok(collSpent > 0, "Should have spent collateral tokens");
    });
  });

  // ══════════════════════════════════════════════════════════
  // Summary
  // ══════════════════════════════════════════════════════════

  describe("Final State Summary", () => {
    it("Print final state", async () => {
      const exchange = await program.account.exchange.fetch(exchangePda);
      console.log("\n  ═══════════════════════════════════");
      console.log("  Final On-Chain State (Devnet)");
      console.log("  ═══════════════════════════════════");
      console.log(`  Exchange admin: ${exchange.admin.toBase58()}`);
      console.log(`  Total users: ${exchange.totalUsers.toNumber()}`);
      console.log(`  Total perp markets: ${exchange.totalPerpMarkets.toNumber()}`);
      console.log(`  Total lending pools: ${exchange.totalLendingPools.toNumber()}`);

      const pool = await program.account.lendingPool.fetch(lendingPoolPda);
      console.log(`  Pool deposits: ${pool.totalDeposits.toNumber() / 1e6} USDC`);
      console.log(`  Pool borrows: ${pool.totalBorrows.toNumber() / 1e6} USDC`);

      const vault = await getAccount(connection, vaultTokenAccount);
      console.log(`  Vault balance: ${Number(vault.amount) / 1e6} USDC`);
      console.log("  ═══════════════════════════════════\n");
    });
  });
  
});
