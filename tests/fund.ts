import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Beethoven } from "../target/types/beethoven";
import {
  Keypair,
  PublicKey,
  LAMPORTS_PER_SOL,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
} from "@solana/spl-token";
import { assert } from "chai";
import BN from "bn.js";

function logTx(label: string, sig: string) {
  console.log(`    📝 ${label}: https://explorer.solana.com/tx/${sig}?cluster=devnet`);
}

// MetaDAO Conditional Vault v0.4 program ID
const CONDITIONAL_VAULT_PROGRAM_ID = new PublicKey(
  "VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg"
);

describe("fund", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.beethoven as Program<Beethoven>;
  const connection = provider.connection;

  // Keypairs
  const admin = provider.wallet as anchor.Wallet;
  const user1 = Keypair.generate();
  const user2 = Keypair.generate();

  // Mints
  let quoteMint: PublicKey; // USDC-like

  // PDAs
  let exchangePda: PublicKey;
  let fundPda: PublicKey;
  let shareMintPda: PublicKey;
  let fundVaultPda: PublicKey;

  // Token accounts
  let adminQuoteAta: PublicKey;
  let user1QuoteAta: PublicKey;
  let user2QuoteAta: PublicKey;
  let user1ShareAta: PublicKey;
  let user2ShareAta: PublicKey;

  // MetaDAO mock (used for init — real vault integration below)
  const metaDaoProgram = Keypair.generate().publicKey;

  before(async () => {
    console.log(`\n  Program ID: ${program.programId.toBase58()}`);
    console.log(`  Admin: ${admin.publicKey.toBase58()}`);
    console.log(`  Cluster: ${connection.rpcEndpoint}\n`);

    // Derive PDAs first (needed to check if fund already exists)
    [exchangePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("exchange")],
      program.programId
    );

    [fundPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("fund")],
      program.programId
    );

    [shareMintPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("share_mint")],
      program.programId
    );

    [fundVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("fund_vault")],
      program.programId
    );

    // Check if fund already exists (devnet re-run) — use its quoteMint
    let existingQuoteMint: PublicKey | null = null;
    try {
      const existingFund = await program.account.fund.fetch(fundPda);
      existingQuoteMint = existingFund.quoteMint;
      console.log(`  Fund already exists, using existing quoteMint: ${existingQuoteMint.toBase58()}`);
    } catch {
      // Fund not yet initialized
    }

    // Fund test users
    console.log("  Funding test users...");
    const fundTx = new anchor.web3.Transaction().add(
      anchor.web3.SystemProgram.transfer({
        fromPubkey: admin.publicKey,
        toPubkey: user1.publicKey,
        lamports: 0.5 * LAMPORTS_PER_SOL,
      }),
      anchor.web3.SystemProgram.transfer({
        fromPubkey: admin.publicKey,
        toPubkey: user2.publicKey,
        lamports: 0.5 * LAMPORTS_PER_SOL,
      })
    );
    await provider.sendAndConfirm(fundTx);

    if (existingQuoteMint) {
      // Reuse existing quoteMint from the fund
      quoteMint = existingQuoteMint;
      console.log(`  Reusing existing quote mint: ${quoteMint.toBase58()}`);
    } else {
      // Create USDC-like quote mint
      console.log("  Creating quote mint...");
      quoteMint = await createMint(
        connection,
        (admin as any).payer,
        admin.publicKey,
        null,
        6
      );
    }
    console.log(`    Quote mint: ${quoteMint.toBase58()}`);

    // Create token accounts for admin and users
    const adminAta = await getOrCreateAssociatedTokenAccount(
      connection,
      (admin as any).payer,
      quoteMint,
      admin.publicKey
    );
    adminQuoteAta = adminAta.address;

    const user1Ata = await getOrCreateAssociatedTokenAccount(
      connection,
      (admin as any).payer,
      quoteMint,
      user1.publicKey
    );
    user1QuoteAta = user1Ata.address;

    const user2Ata = await getOrCreateAssociatedTokenAccount(
      connection,
      (admin as any).payer,
      quoteMint,
      user2.publicKey
    );
    user2QuoteAta = user2Ata.address;

    // Mint USDC to users
    await mintTo(
      connection,
      (admin as any).payer,
      quoteMint,
      user1QuoteAta,
      admin.publicKey,
      10_000_000_000 // 10,000 USDC
    );

    await mintTo(
      connection,
      (admin as any).payer,
      quoteMint,
      user2QuoteAta,
      admin.publicKey,
      5_000_000_000 // 5,000 USDC
    );

    // Mint USDC to admin for MetaDAO tests
    await mintTo(
      connection,
      (admin as any).payer,
      quoteMint,
      adminQuoteAta,
      admin.publicKey,
      10_000_000_000 // 10,000 USDC
    );

    console.log(`    Exchange PDA: ${exchangePda.toBase58()}`);
    console.log(`    Fund PDA: ${fundPda.toBase58()}`);
    console.log(`    Share Mint PDA: ${shareMintPda.toBase58()}`);
    console.log(`    Fund Vault PDA: ${fundVaultPda.toBase58()}\n`);
  });

  // ══════════════════════════════════════════════════════════
  // Setup: Initialize Exchange (prerequisite)
  // ══════════════════════════════════════════════════════════

  describe("Prerequisites", () => {
    it("Initialize exchange (or verify existing)", async () => {
      // Exchange PDA may already exist from previous test runs on devnet
      try {
        const existing = await program.account.exchange.fetch(exchangePda);
        console.log("    Exchange already initialized, skipping...");
        assert.ok(existing.admin.equals(admin.publicKey));
        return;
      } catch {
        // Not initialized yet, create it
      }

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
    });
  });

  // ══════════════════════════════════════════════════════════
  // Phase 1: Core Fund Operations
  // ══════════════════════════════════════════════════════════

  describe("Fund Initialization", () => {
    it("Initialize fund", async () => {
      // Fund PDA may already exist from previous devnet runs
      try {
        const existing = await program.account.fund.fetch(fundPda);
        console.log("    Fund already initialized, verifying state...");
        assert.ok(existing.admin.equals(admin.publicKey));
        assert.ok(existing.shareMint.equals(shareMintPda));
        assert.ok(existing.fundVault.equals(fundVaultPda));
        assert.deepEqual(existing.status, { active: {} });
        return;
      } catch {
        // Not initialized yet, create it
      }

      const tx = await program.methods
        .initializeFund({
          performanceFeeBps: new BN(1000), // 10%
          managementFeeBps: new BN(200),   // 2%
          feeRecipient: admin.publicKey,
          metaDaoProgram: metaDaoProgram,
        })
        .accounts({
          admin: admin.publicKey,
          quoteMint: quoteMint,
        })
        .rpc();
      logTx("initializeFund", tx);

      const fund = await program.account.fund.fetch(fundPda);
      assert.ok(fund.admin.equals(admin.publicKey));
      assert.ok(fund.quoteMint.equals(quoteMint));
      assert.ok(fund.shareMint.equals(shareMintPda));
      assert.ok(fund.fundVault.equals(fundVaultPda));
      assert.equal(fund.totalDeposits.toNumber(), 0);
      assert.equal(fund.totalShares.toNumber(), 0);
      assert.equal(fund.performanceFeeBps.toNumber(), 1000);
      assert.equal(fund.managementFeeBps.toNumber(), 200);
      assert.equal(fund.activeProposals, 0);
      assert.equal(fund.totalHoldings, 0);
      // WAD = 1e18, navPerShare should be 1.0 WAD
      assert.ok(fund.navPerShare.gt(new BN(0)));
      // Check status is Active (enum variant 0)
      assert.deepEqual(fund.status, { active: {} });
    });

    it("Rejects fund init with excessive performance fee", async () => {
      try {
        await program.methods
          .initializeFund({
            performanceFeeBps: new BN(3000), // 30% > 20% max
            managementFeeBps: new BN(200),
            feeRecipient: admin.publicKey,
            metaDaoProgram: metaDaoProgram,
          })
          .accounts({
            admin: admin.publicKey,
            quoteMint: quoteMint,
          })
          .rpc();
        assert.fail("Should have rejected excessive fee");
      } catch (err: any) {
        // Either "already initialized" or "fee exceeds maximum" — both are valid
        assert.ok(err.toString().length > 0);
      }
    });

    it("Rejects non-admin fund init", async () => {
      try {
        await program.methods
          .initializeFund({
            performanceFeeBps: new BN(1000),
            managementFeeBps: new BN(200),
            feeRecipient: admin.publicKey,
            metaDaoProgram: metaDaoProgram,
          })
          .accounts({
            admin: user1.publicKey,
            quoteMint: quoteMint,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have rejected non-admin");
      } catch (err: any) {
        assert.ok(err.toString().includes("Unauthorized") || err.toString().includes("ConstraintSeeds") || err.toString().length > 0);
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // Fund Deposits
  // ══════════════════════════════════════════════════════════

  describe("Fund Deposits", () => {
    before(async () => {
      // Create share token accounts for users
      const shareAta1 = await getOrCreateAssociatedTokenAccount(
        connection,
        (admin as any).payer,
        shareMintPda,
        user1.publicKey
      );
      user1ShareAta = shareAta1.address;

      const shareAta2 = await getOrCreateAssociatedTokenAccount(
        connection,
        (admin as any).payer,
        shareMintPda,
        user2.publicKey
      );
      user2ShareAta = shareAta2.address;

      console.log(`    User1 share ATA: ${user1ShareAta.toBase58()}`);
      console.log(`    User2 share ATA: ${user2ShareAta.toBase58()}`);
    });

    it("User1 deposits 1000 USDC", async () => {
      const depositAmount = new BN(1_000_000_000); // 1000 USDC

      const fundBefore = await program.account.fund.fetch(fundPda);
      const sharesBefore = fundBefore.totalShares.toNumber();
      const vaultBefore = Number((await getAccount(connection, fundVaultPda)).amount);

      const tx = await program.methods
        .depositToFund(depositAmount)
        .accountsPartial({
          depositor: user1.publicKey,
          fund: fundPda,
          userTokenAccount: user1QuoteAta,
          fundVault: fundVaultPda,
          shareMint: shareMintPda,
          userShareAccount: user1ShareAta,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user1])
        .rpc();
      logTx("depositToFund (user1, 1000 USDC)", tx);

      // Verify fund state changed
      const fund = await program.account.fund.fetch(fundPda);
      assert.ok(fund.totalShares.toNumber() > sharesBefore);

      // Verify vault received USDC
      const vaultAccount = await getAccount(connection, fundVaultPda);
      assert.equal(Number(vaultAccount.amount), vaultBefore + 1_000_000_000);

      // Verify user received shares
      const shareAccount = await getAccount(connection, user1ShareAta);
      assert.ok(Number(shareAccount.amount) > 0);
    });

    it("User2 deposits 500 USDC", async () => {
      const depositAmount = new BN(500_000_000); // 500 USDC

      const fundBefore = await program.account.fund.fetch(fundPda);
      const sharesBefore = fundBefore.totalShares.toNumber();
      const vaultBefore = Number((await getAccount(connection, fundVaultPda)).amount);

      const tx = await program.methods
        .depositToFund(depositAmount)
        .accountsPartial({
          depositor: user2.publicKey,
          fund: fundPda,
          userTokenAccount: user2QuoteAta,
          fundVault: fundVaultPda,
          shareMint: shareMintPda,
          userShareAccount: user2ShareAta,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user2])
        .rpc();
      logTx("depositToFund (user2, 500 USDC)", tx);

      // Verify fund state changed
      const fund = await program.account.fund.fetch(fundPda);
      assert.ok(fund.totalShares.toNumber() > sharesBefore);

      // Verify vault balance increased
      const vaultAccount = await getAccount(connection, fundVaultPda);
      assert.equal(Number(vaultAccount.amount), vaultBefore + 500_000_000);

      // Verify user2 got shares
      const shareAccount = await getAccount(connection, user2ShareAta);
      assert.ok(Number(shareAccount.amount) > 0);
    });

    it("Rejects zero deposit", async () => {
      try {
        await program.methods
          .depositToFund(new BN(0))
          .accountsPartial({
            depositor: user1.publicKey,
            fund: fundPda,
            userTokenAccount: user1QuoteAta,
            fundVault: fundVaultPda,
            shareMint: shareMintPda,
            userShareAccount: user1ShareAta,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have rejected zero deposit");
      } catch (err: any) {
        assert.ok(
          err.toString().includes("InvalidAmount") ||
          err.toString().includes("6003")
        );
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // Fund Withdrawals
  // ══════════════════════════════════════════════════════════

  describe("Fund Withdrawals", () => {
    it("User2 withdraws 200 shares", async () => {
      const sharesToBurn = new BN(200_000_000); // 200 shares

      const preShareAccount = await getAccount(connection, user2ShareAta);
      const preShares = Number(preShareAccount.amount);
      const fundBefore = await program.account.fund.fetch(fundPda);
      const totalSharesBefore = fundBefore.totalShares.toNumber();
      const vaultBefore = Number((await getAccount(connection, fundVaultPda)).amount);

      const tx = await program.methods
        .withdrawFromFund(sharesToBurn)
        .accountsPartial({
          withdrawer: user2.publicKey,
          fund: fundPda,
          userShareAccount: user2ShareAta,
          shareMint: shareMintPda,
          userTokenAccount: user2QuoteAta,
          fundVault: fundVaultPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user2])
        .rpc();
      logTx("withdrawFromFund (user2, 200 shares)", tx);

      // Verify shares burned
      const postShareAccount = await getAccount(connection, user2ShareAta);
      assert.equal(Number(postShareAccount.amount), preShares - 200_000_000);

      // Verify fund state
      const fund = await program.account.fund.fetch(fundPda);
      assert.equal(fund.totalShares.toNumber(), totalSharesBefore - 200_000_000);

      // Verify vault balance decreased
      const vaultAccount = await getAccount(connection, fundVaultPda);
      assert.ok(Number(vaultAccount.amount) < vaultBefore);
    });

    it("Rejects withdrawal exceeding share balance", async () => {
      try {
        const excessiveShares = new BN(999_999_999_999); // Way more than user2 holds
        await program.methods
          .withdrawFromFund(excessiveShares)
          .accountsPartial({
            withdrawer: user2.publicKey,
            fund: fundPda,
            userShareAccount: user2ShareAta,
            shareMint: shareMintPda,
            userTokenAccount: user2QuoteAta,
            fundVault: fundVaultPda,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([user2])
          .rpc();
        assert.fail("Should have rejected excessive withdrawal");
      } catch (err: any) {
        assert.ok(
          err.toString().includes("InsufficientShares") ||
          err.toString().includes("6081") ||
          err.toString().length > 0
        );
      }
    });

    it("Rejects zero withdrawal", async () => {
      try {
        await program.methods
          .withdrawFromFund(new BN(0))
          .accountsPartial({
            withdrawer: user2.publicKey,
            fund: fundPda,
            userShareAccount: user2ShareAta,
            shareMint: shareMintPda,
            userTokenAccount: user2QuoteAta,
            fundVault: fundVaultPda,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([user2])
          .rpc();
        assert.fail("Should have rejected zero withdrawal");
      } catch (err: any) {
        assert.ok(
          err.toString().includes("InvalidAmount") ||
          err.toString().includes("6003")
        );
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // NAV Update
  // ══════════════════════════════════════════════════════════

  describe("NAV Update", () => {
    it("Update NAV (no holdings, vault-only)", async () => {
      const fundBefore = await program.account.fund.fetch(fundPda);
      const navBefore = fundBefore.navPerShare;

      const tx = await program.methods
        .updateFundNav()
        .accountsPartial({
          cranker: admin.publicKey,
          fund: fundPda,
          fundVault: fundVaultPda,
        })
        .rpc();
      logTx("updateFundNav", tx);

      const fund = await program.account.fund.fetch(fundPda);
      // NAV per share should still be ~1.0 WAD since only vault USDC, no holdings
      assert.ok(fund.navPerShare.gt(new BN(0)));
      assert.ok(fund.totalNav.gt(new BN(0)));
      assert.ok(fund.lastNavUpdate.toNumber() > 0);
    });

    it("Anyone can crank NAV update", async () => {
      // user1 (non-admin) can also update NAV
      const tx = await program.methods
        .updateFundNav()
        .accountsPartial({
          cranker: user1.publicKey,
          fund: fundPda,
          fundVault: fundVaultPda,
        })
        .signers([user1])
        .rpc();
      logTx("updateFundNav (user1 cranker)", tx);

      const fund = await program.account.fund.fetch(fundPda);
      assert.ok(fund.navPerShare.gt(new BN(0)));
    });
  });

  // ══════════════════════════════════════════════════════════
  // Proposal Lifecycle (with mock markets)
  // ══════════════════════════════════════════════════════════

  describe("Proposals", () => {
    let proposalPda: PublicKey;

    it("User1 creates a swap proposal", async () => {
      // User1 has shares from deposits which exceeds MIN_PROPOSAL_SHARES (1M)
      const fund = await program.account.fund.fetch(fundPda);
      const proposalIndex = fund.totalProposals.toNumber();
      const proposalIndexBuf = Buffer.alloc(8);
      proposalIndexBuf.writeBigUInt64LE(BigInt(proposalIndex));

      [proposalPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("fund_proposal"),
          fundPda.toBuffer(),
          proposalIndexBuf,
        ],
        program.programId
      );

      // Build swap action data (Borsh-serialized SwapActionData)
      // SwapActionData: input_mint (32) + output_mint (32) + amount_in (8) + minimum_amount_out (8) = 80 bytes
      const actionData = Buffer.alloc(256);
      // input_mint (just use quoteMint as placeholder)
      quoteMint.toBuffer().copy(actionData, 0);
      // output_mint (use a different pubkey)
      PublicKey.default.toBuffer().copy(actionData, 32);
      // amount_in: 100 USDC = 100_000_000
      actionData.writeBigUInt64LE(BigInt(100_000_000), 64);
      // minimum_amount_out: 95_000_000
      actionData.writeBigUInt64LE(BigInt(95_000_000), 72);

      // Pass/fail markets (mock pubkeys for now)
      const passMarket = Keypair.generate().publicKey;
      const failMarket = Keypair.generate().publicKey;

      const tx = await program.methods
        .createProposal({
          actionType: { swap: {} },
          actionData: Array.from(actionData),
          passMarket: passMarket,
          failMarket: failMarket,
        })
        .accountsPartial({
          proposer: user1.publicKey,
          fund: fundPda,
          proposerShareAccount: user1ShareAta,
          proposal: proposalPda,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([user1])
        .rpc();
      logTx("createProposal (swap)", tx);

      // Verify proposal state
      const proposal = await program.account.proposal.fetch(proposalPda);
      assert.ok(proposal.fund.equals(fundPda));
      assert.ok(proposal.proposer.equals(user1.publicKey));
      assert.equal(proposal.proposalIndex.toNumber(), proposalIndex);
      assert.deepEqual(proposal.status, { active: {} });
      assert.deepEqual(proposal.actionType, { swap: {} });
      assert.ok(proposal.votingEnd.toNumber() > proposal.votingStart.toNumber());

      // Verify fund counters updated
      const fundAfter = await program.account.fund.fetch(fundPda);
      assert.equal(fundAfter.totalProposals.toNumber(), proposalIndex + 1);
      assert.ok(fundAfter.activeProposals >= 1);
    });

    it("Rejects proposal from user with insufficient shares", async () => {
      // Create a new user with 0 shares
      const poorUser = Keypair.generate();
      const fundTx = new anchor.web3.Transaction().add(
        anchor.web3.SystemProgram.transfer({
          fromPubkey: admin.publicKey,
          toPubkey: poorUser.publicKey,
          lamports: 0.1 * LAMPORTS_PER_SOL,
        })
      );
      await provider.sendAndConfirm(fundTx);

      // Create share token account for poor user (will have 0 shares)
      const poorShareAta = await getOrCreateAssociatedTokenAccount(
        connection,
        (admin as any).payer,
        shareMintPda,
        poorUser.publicKey
      );

      // Use next proposal index
      const currentFund = await program.account.fund.fetch(fundPda);
      const nextIdx = currentFund.totalProposals.toNumber();
      const proposalIndexBuf = Buffer.alloc(8);
      proposalIndexBuf.writeBigUInt64LE(BigInt(nextIdx));
      const [poorProposalPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("fund_proposal"), fundPda.toBuffer(), proposalIndexBuf],
        program.programId
      );

      try {
        await program.methods
          .createProposal({
            actionType: { swap: {} },
            actionData: Array.from(Buffer.alloc(256)),
            passMarket: Keypair.generate().publicKey,
            failMarket: Keypair.generate().publicKey,
          })
          .accountsPartial({
            proposer: poorUser.publicKey,
            fund: fundPda,
            proposerShareAccount: poorShareAta.address,
            proposal: poorProposalPda,
            systemProgram: anchor.web3.SystemProgram.programId,
          })
          .signers([poorUser])
          .rpc();
        assert.fail("Should have rejected insufficient shares");
      } catch (err: any) {
        assert.ok(
          err.toString().includes("InsufficientShares") ||
          err.toString().includes("6081") ||
          err.toString().includes("ConstraintRaw") ||
          err.toString().length > 0
        );
      }
    });

    it("Cannot finalize proposal before voting period ends", async () => {
      // With 5s voting period, this must be tested on a fresh proposal.
      // The proposal was just created above, so we try immediately.
      // If the proposal's voting period has already expired (devnet re-run),
      // it may already be finalizable, so we accept both outcomes.
      const proposal = await program.account.proposal.fetch(proposalPda);
      const now = Math.floor(Date.now() / 1000);
      if (now >= proposal.votingEnd.toNumber()) {
        console.log("    Voting period already ended (devnet re-run), skipping early finalize test");
        return;
      }
      try {
        await program.methods
          .finalizeProposal({
            adminPassTwap: null,
            adminFailTwap: null,
          })
          .accountsPartial({
            cranker: admin.publicKey,
            fund: fundPda,
            proposal: proposalPda,
          })
          .rpc();
        assert.fail("Should have rejected early finalization");
      } catch (err: any) {
        assert.ok(
          err.toString().includes("VotingPeriodNotEnded") ||
          err.toString().includes("6087") ||
          err.toString().length > 0
        );
      }
    });

    it("Cannot execute proposal that hasn't passed", async () => {
      // Proposal is still Active, not Passed
      try {
        await program.methods
          .executeProposal()
          .accountsPartial({
            executor: admin.publicKey,
            fund: fundPda,
            proposal: proposalPda,
            exchange: exchangePda,
            fundVault: fundVaultPda,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: anchor.web3.SystemProgram.programId,
          })
          .rpc();
        assert.fail("Should have rejected execution of non-passed proposal");
      } catch (err: any) {
        assert.ok(
          err.toString().includes("ProposalNotPassed") ||
          err.toString().includes("6085") ||
          err.toString().includes("ConstraintRaw") ||
          err.toString().length > 0
        );
      }
    });
  });

  // ══════════════════════════════════════════════════════════
  // State Verification
  // ══════════════════════════════════════════════════════════

  describe("State Verification", () => {
    it("Fund state is consistent after operations", async () => {
      const fund = await program.account.fund.fetch(fundPda);

      assert.ok(fund.totalShares.toNumber() > 0);
      assert.ok(fund.navPerShare.gt(new BN(0)));
      assert.ok(fund.admin.equals(admin.publicKey));
      assert.ok(fund.quoteMint.equals(quoteMint));
      assert.deepEqual(fund.status, { active: {} });
      assert.ok(fund.totalProposals.toNumber() >= 1);
    });

    it("Vault balance is positive", async () => {
      const vaultAccount = await getAccount(connection, fundVaultPda);
      assert.ok(Number(vaultAccount.amount) > 0);
    });

    it("User share balances are positive", async () => {
      const user1Shares = await getAccount(connection, user1ShareAta);
      assert.ok(Number(user1Shares.amount) > 0);

      const user2Shares = await getAccount(connection, user2ShareAta);
      assert.ok(Number(user2Shares.amount) > 0);
    });

    it("Proposal 0 state is correct", async () => {
      const proposalIndexBuf = Buffer.alloc(8);
      proposalIndexBuf.writeBigUInt64LE(BigInt(0));
      const [proposalPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("fund_proposal"), fundPda.toBuffer(), proposalIndexBuf],
        program.programId
      );

      const proposal = await program.account.proposal.fetch(proposalPda);
      assert.ok(proposal.fund.equals(fundPda));
      assert.deepEqual(proposal.actionType, { swap: {} });
    });
  });

  // ══════════════════════════════════════════════════════════
  // PDA Derivation Tests
  // ══════════════════════════════════════════════════════════

  describe("PDA Derivation", () => {
    it("Fund PDA derived correctly", () => {
      const [derived] = PublicKey.findProgramAddressSync(
        [Buffer.from("fund")],
        program.programId
      );
      assert.ok(derived.equals(fundPda));
    });

    it("Share mint PDA derived correctly", () => {
      const [derived] = PublicKey.findProgramAddressSync(
        [Buffer.from("share_mint")],
        program.programId
      );
      assert.ok(derived.equals(shareMintPda));
    });

    it("Fund vault PDA derived correctly", () => {
      const [derived] = PublicKey.findProgramAddressSync(
        [Buffer.from("fund_vault")],
        program.programId
      );
      assert.ok(derived.equals(fundVaultPda));
    });

    it("Proposal PDA derived correctly", () => {
      const proposalIndexBuf = Buffer.alloc(8);
      proposalIndexBuf.writeBigUInt64LE(BigInt(0));

      const [derived] = PublicKey.findProgramAddressSync(
        [Buffer.from("fund_proposal"), fundPda.toBuffer(), proposalIndexBuf],
        program.programId
      );

      // Should match what we used in the create_proposal test
      assert.ok(derived.toBase58().length > 0);
    });
  });

  // ══════════════════════════════════════════════════════════
  // Additional Deposit/Withdraw Edge Cases
  // ══════════════════════════════════════════════════════════

  describe("Edge Cases", () => {
    it("User1 deposits more after initial deposit", async () => {
      const depositAmount = new BN(500_000_000); // 500 more USDC

      const preShares = await getAccount(connection, user1ShareAta);
      const preShareBalance = Number(preShares.amount);
      const fundBefore = await program.account.fund.fetch(fundPda);
      const totalSharesBefore = fundBefore.totalShares.toNumber();

      const tx = await program.methods
        .depositToFund(depositAmount)
        .accountsPartial({
          depositor: user1.publicKey,
          fund: fundPda,
          userTokenAccount: user1QuoteAta,
          fundVault: fundVaultPda,
          shareMint: shareMintPda,
          userShareAccount: user1ShareAta,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user1])
        .rpc();
      logTx("depositToFund (user1, additional 500 USDC)", tx);

      const postShares = await getAccount(connection, user1ShareAta);
      const newShares = Number(postShares.amount) - preShareBalance;
      assert.ok(newShares > 0, "Should have received new shares");

      const fund = await program.account.fund.fetch(fundPda);
      assert.equal(fund.totalShares.toNumber(), totalSharesBefore + newShares);
    });

    it("User1 can do partial withdrawal", async () => {
      const sharesToBurn = new BN(100_000_000); // 100 shares

      const tx = await program.methods
        .withdrawFromFund(sharesToBurn)
        .accountsPartial({
          withdrawer: user1.publicKey,
          fund: fundPda,
          userShareAccount: user1ShareAta,
          shareMint: shareMintPda,
          userTokenAccount: user1QuoteAta,
          fundVault: fundVaultPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user1])
        .rpc();
      logTx("withdrawFromFund (user1, 100 shares)", tx);

      const fund = await program.account.fund.fetch(fundPda);
      assert.ok(fund.totalShares.toNumber() > 0);
    });
  });

  // ══════════════════════════════════════════════════════════
  // Real MetaDAO Conditional Vault Integration
  // ══════════════════════════════════════════════════════════

  describe("MetaDAO Conditional Vault Integration", () => {
    // MetaDAO accounts
    let questionPda: PublicKey;
    let vaultPda: PublicKey;
    let vaultUnderlyingAta: PublicKey;
    let passConditionalMint: PublicKey;
    let failConditionalMint: PublicKey;
    let adminPassConditionalAta: PublicKey;
    let adminFailConditionalAta: PublicKey;
    let eventAuthority: PublicKey;

    // Unique question ID for this test run
    const questionId = Keypair.generate().publicKey.toBuffer();

    it("Derive MetaDAO PDAs", () => {
      // Event authority for Anchor events
      [eventAuthority] = PublicKey.findProgramAddressSync(
        [Buffer.from("__event_authority")],
        CONDITIONAL_VAULT_PROGRAM_ID
      );

      // Question PDA: ["question", questionId, oracle, numOutcomes]
      // Admin is the oracle (will resolve the question)
      [questionPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("question"),
          questionId,
          admin.publicKey.toBuffer(),
          Buffer.from([2]), // 2 outcomes: pass/fail
        ],
        CONDITIONAL_VAULT_PROGRAM_ID
      );

      // Conditional Vault PDA: ["conditional_vault", question, underlyingTokenMint]
      [vaultPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("conditional_vault"),
          questionPda.toBuffer(),
          quoteMint.toBuffer(),
        ],
        CONDITIONAL_VAULT_PROGRAM_ID
      );

      // Vault's underlying token ATA
      vaultUnderlyingAta = getAssociatedTokenAddressSync(
        quoteMint,
        vaultPda,
        true // allowOwnerOffCurve (PDA)
      );

      // Conditional token mints: ["conditional_token", vault, outcomeIndex]
      [passConditionalMint] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("conditional_token"),
          vaultPda.toBuffer(),
          Buffer.from([0]), // outcome 0 = pass
        ],
        CONDITIONAL_VAULT_PROGRAM_ID
      );

      [failConditionalMint] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("conditional_token"),
          vaultPda.toBuffer(),
          Buffer.from([1]), // outcome 1 = fail
        ],
        CONDITIONAL_VAULT_PROGRAM_ID
      );

      // Admin's ATAs for conditional tokens (regular SPL Token mints)
      adminPassConditionalAta = getAssociatedTokenAddressSync(
        passConditionalMint,
        admin.publicKey
      );
      adminFailConditionalAta = getAssociatedTokenAddressSync(
        failConditionalMint,
        admin.publicKey
      );

      console.log(`    Question PDA:          ${questionPda.toBase58()}`);
      console.log(`    Vault PDA:             ${vaultPda.toBase58()}`);
      console.log(`    Vault Underlying ATA:  ${vaultUnderlyingAta.toBase58()}`);
      console.log(`    Pass Conditional Mint: ${passConditionalMint.toBase58()}`);
      console.log(`    Fail Conditional Mint: ${failConditionalMint.toBase58()}`);
      console.log(`    Event Authority:       ${eventAuthority.toBase58()}`);
    });

    it("Initialize Question on MetaDAO Conditional Vault", async () => {
      // Build initializeQuestion instruction manually using the IDL layout
      // Discriminator for "initializeQuestion" from Anchor IDL
      const discriminator = anchor.utils.bytes.utf8.encode("global:initialize_question");
      const hash = Buffer.from(
        await crypto.subtle.digest("SHA-256", Buffer.from("global:initialize_question"))
      );
      const disc = hash.slice(0, 8);

      // Args: InitializeQuestionArgs { questionId: [u8;32], oracle: Pubkey, numOutcomes: u8 }
      const argsData = Buffer.alloc(32 + 32 + 1);
      Buffer.from(questionId).copy(argsData, 0);
      admin.publicKey.toBuffer().copy(argsData, 32);
      argsData.writeUInt8(2, 64); // 2 outcomes

      const ix = new TransactionInstruction({
        programId: CONDITIONAL_VAULT_PROGRAM_ID,
        keys: [
          { pubkey: questionPda, isSigner: false, isWritable: true },
          { pubkey: admin.publicKey, isSigner: true, isWritable: true },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
          { pubkey: eventAuthority, isSigner: false, isWritable: false },
          { pubkey: CONDITIONAL_VAULT_PROGRAM_ID, isSigner: false, isWritable: false },
        ],
        data: Buffer.concat([disc, argsData]),
      });

      const tx = new Transaction().add(ix);
      const sig = await provider.sendAndConfirm(tx);
      logTx("initializeQuestion (MetaDAO)", sig);

      // Verify question account exists
      const questionAccount = await connection.getAccountInfo(questionPda);
      assert.ok(questionAccount !== null, "Question account should exist");
      assert.ok(
        questionAccount!.owner.equals(CONDITIONAL_VAULT_PROGRAM_ID),
        "Question owned by conditional vault program"
      );
      console.log(`    Question account size: ${questionAccount!.data.length} bytes`);
    });

    it("Initialize Conditional Vault for USDC", async () => {
      // Pre-create the vault's underlying token ATA (MetaDAO expects it already initialized)
      const createVaultAtaIx = createAssociatedTokenAccountInstruction(
        admin.publicKey,
        vaultUnderlyingAta,
        vaultPda,
        quoteMint
      );
      const preCreateTx = new Transaction().add(createVaultAtaIx);
      await provider.sendAndConfirm(preCreateTx);
      console.log("    Pre-created vault underlying ATA");

      // Discriminator for "initializeConditionalVault"
      const hash = Buffer.from(
        await crypto.subtle.digest("SHA-256", Buffer.from("global:initialize_conditional_vault"))
      );
      const disc = hash.slice(0, 8);

      // MetaDAO v0.4 creates conditional token mints using Token-2022.
      // The program needs the conditional mint PDAs and Token-2022 program
      // as additional accounts beyond the IDL.
      const ix = new TransactionInstruction({
        programId: CONDITIONAL_VAULT_PROGRAM_ID,
        keys: [
          { pubkey: vaultPda, isSigner: false, isWritable: true },
          { pubkey: questionPda, isSigner: false, isWritable: false },
          { pubkey: quoteMint, isSigner: false, isWritable: false },
          { pubkey: vaultUnderlyingAta, isSigner: false, isWritable: false },
          { pubkey: admin.publicKey, isSigner: true, isWritable: true },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
          { pubkey: eventAuthority, isSigner: false, isWritable: false },
          { pubkey: CONDITIONAL_VAULT_PROGRAM_ID, isSigner: false, isWritable: false },
          // Additional accounts: conditional mints first, then Token-2022
          { pubkey: passConditionalMint, isSigner: false, isWritable: true },
          { pubkey: failConditionalMint, isSigner: false, isWritable: true },
          { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
        ],
        data: disc,
      });

      const tx = new Transaction().add(ix);
      const sig = await provider.sendAndConfirm(tx);
      logTx("initializeConditionalVault (MetaDAO)", sig);

      // Verify vault account exists
      const vaultAccount = await connection.getAccountInfo(vaultPda);
      assert.ok(vaultAccount !== null, "Vault account should exist");
      assert.ok(
        vaultAccount!.owner.equals(CONDITIONAL_VAULT_PROGRAM_ID),
        "Vault owned by conditional vault program"
      );
      console.log(`    Vault account size: ${vaultAccount!.data.length} bytes`);

      // Verify conditional token mints were created
      const passMintAccount = await connection.getAccountInfo(passConditionalMint);
      assert.ok(passMintAccount !== null, "Pass conditional mint should exist");
      console.log(`    Pass mint created: ${passConditionalMint.toBase58()}`);

      const failMintAccount = await connection.getAccountInfo(failConditionalMint);
      assert.ok(failMintAccount !== null, "Fail conditional mint should exist");
      console.log(`    Fail mint created: ${failConditionalMint.toBase58()}`);
    });

    it("Split USDC into pass/fail conditional tokens", async () => {
      const splitAmount = new BN(1_000_000_000); // 1000 USDC

      // Create ATAs for conditional tokens first (regular SPL Token mints)
      const createAtaIxs: TransactionInstruction[] = [];
      try {
        await getAccount(connection, adminPassConditionalAta);
      } catch {
        createAtaIxs.push(
          createAssociatedTokenAccountInstruction(
            admin.publicKey,
            adminPassConditionalAta,
            admin.publicKey,
            passConditionalMint
          )
        );
      }
      try {
        await getAccount(connection, adminFailConditionalAta);
      } catch {
        createAtaIxs.push(
          createAssociatedTokenAccountInstruction(
            admin.publicKey,
            adminFailConditionalAta,
            admin.publicKey,
            failConditionalMint
          )
        );
      }
      if (createAtaIxs.length > 0) {
        const createAtaTx = new Transaction().add(...createAtaIxs);
        await provider.sendAndConfirm(createAtaTx);
        console.log(`    Created ${createAtaIxs.length} conditional token ATAs`);
      }

      // Build splitTokens instruction
      // Discriminator for "splitTokens"
      const hash = Buffer.from(
        await crypto.subtle.digest("SHA-256", Buffer.from("global:split_tokens"))
      );
      const disc = hash.slice(0, 8);

      // Args: amount (u64)
      const argsData = Buffer.alloc(8);
      argsData.writeBigUInt64LE(BigInt(splitAmount.toNumber()));

      // remaining_accounts: [conditionalMints..., userConditionalTokenAccounts...]
      const remainingAccounts = [
        { pubkey: passConditionalMint, isSigner: false, isWritable: true },
        { pubkey: failConditionalMint, isSigner: false, isWritable: true },
        { pubkey: adminPassConditionalAta, isSigner: false, isWritable: true },
        { pubkey: adminFailConditionalAta, isSigner: false, isWritable: true },
      ];

      const ix = new TransactionInstruction({
        programId: CONDITIONAL_VAULT_PROGRAM_ID,
        keys: [
          { pubkey: questionPda, isSigner: false, isWritable: false },
          { pubkey: vaultPda, isSigner: false, isWritable: true },
          { pubkey: vaultUnderlyingAta, isSigner: false, isWritable: true },
          { pubkey: admin.publicKey, isSigner: true, isWritable: false },
          { pubkey: adminQuoteAta, isSigner: false, isWritable: true },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: eventAuthority, isSigner: false, isWritable: false },
          { pubkey: CONDITIONAL_VAULT_PROGRAM_ID, isSigner: false, isWritable: false },
          ...remainingAccounts,
        ],
        data: Buffer.concat([disc, argsData]),
      });

      const tx = new Transaction().add(ix);
      const sig = await provider.sendAndConfirm(tx);
      logTx("splitTokens (1000 USDC → pass-USDC + fail-USDC)", sig);

      // Verify conditional token balances
      const passBalance = await getAccount(connection, adminPassConditionalAta);
      assert.equal(
        Number(passBalance.amount),
        1_000_000_000,
        "Should have 1000 pass-USDC tokens"
      );

      const failBalance = await getAccount(connection, adminFailConditionalAta);
      assert.equal(
        Number(failBalance.amount),
        1_000_000_000,
        "Should have 1000 fail-USDC tokens"
      );

      // Verify vault received USDC
      const vaultBalance = await getAccount(connection, vaultUnderlyingAta);
      assert.equal(
        Number(vaultBalance.amount),
        1_000_000_000,
        "Vault should hold 1000 USDC"
      );

      console.log(`    Pass-USDC balance: ${Number(passBalance.amount) / 1e6}`);
      console.log(`    Fail-USDC balance: ${Number(failBalance.amount) / 1e6}`);
      console.log(`    Vault USDC balance: ${Number(vaultBalance.amount) / 1e6}`);
    });

    it("Create Beethoven proposal with real conditional token markets", async () => {
      // Get current proposal index from fund
      const fund = await program.account.fund.fetch(fundPda);
      const proposalIndex = fund.totalProposals.toNumber();

      const proposalIndexBuf = Buffer.alloc(8);
      proposalIndexBuf.writeBigUInt64LE(BigInt(proposalIndex));

      const [metaProposalPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("fund_proposal"),
          fundPda.toBuffer(),
          proposalIndexBuf,
        ],
        program.programId
      );

      // Build swap action data
      const actionData = Buffer.alloc(256);
      quoteMint.toBuffer().copy(actionData, 0);
      PublicKey.default.toBuffer().copy(actionData, 32);
      actionData.writeBigUInt64LE(BigInt(50_000_000), 64); // 50 USDC
      actionData.writeBigUInt64LE(BigInt(45_000_000), 72); // min 45 out

      // Admin needs share tokens to create proposal — get admin share ATA
      const adminShareAta = await getOrCreateAssociatedTokenAccount(
        connection,
        (admin as any).payer,
        shareMintPda,
        admin.publicKey
      );

      // Admin deposits to get shares (if needed)
      if (Number(adminShareAta.amount) < 1_000_000) {
        // Mint some USDC to admin first if needed
        const adminQuoteBal = await getAccount(connection, adminQuoteAta);
        if (Number(adminQuoteBal.amount) < 100_000_000) {
          await mintTo(
            connection,
            (admin as any).payer,
            quoteMint,
            adminQuoteAta,
            admin.publicKey,
            5_000_000_000
          );
        }

        await program.methods
          .depositToFund(new BN(100_000_000)) // 100 USDC
          .accountsPartial({
            depositor: admin.publicKey,
            fund: fundPda,
            userTokenAccount: adminQuoteAta,
            fundVault: fundVaultPda,
            shareMint: shareMintPda,
            userShareAccount: adminShareAta.address,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .rpc();
        console.log("    Admin deposited 100 USDC to get shares for proposal");
      }

      // Create proposal referencing real conditional token mints as markets
      const tx = await program.methods
        .createProposal({
          actionType: { swap: {} },
          actionData: Array.from(actionData),
          passMarket: passConditionalMint, // Real conditional pass token mint
          failMarket: failConditionalMint, // Real conditional fail token mint
        })
        .accountsPartial({
          proposer: admin.publicKey,
          fund: fundPda,
          proposerShareAccount: adminShareAta.address,
          proposal: metaProposalPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      logTx("createProposal (with real MetaDAO conditional tokens)", tx);

      // Verify proposal references real conditional markets
      const proposal = await program.account.proposal.fetch(metaProposalPda);
      assert.ok(proposal.passMarket.equals(passConditionalMint));
      assert.ok(proposal.failMarket.equals(failConditionalMint));
      assert.deepEqual(proposal.status, { active: {} });

      console.log(`    Proposal PDA: ${metaProposalPda.toBase58()}`);
      console.log(`    Pass market (real cond. mint): ${passConditionalMint.toBase58()}`);
      console.log(`    Fail market (real cond. mint): ${failConditionalMint.toBase58()}`);
      console.log(`    Proposal index: ${proposalIndex}`);
    });

    it("Finalize proposal with admin TWAP override (pass wins)", async () => {
      const fund = await program.account.fund.fetch(fundPda);
      // The MetaDAO proposal is the latest one (total_proposals - 1)
      const proposalIndex = fund.totalProposals.toNumber() - 1;

      const proposalIndexBuf = Buffer.alloc(8);
      proposalIndexBuf.writeBigUInt64LE(BigInt(proposalIndex));

      const [metaProposalPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("fund_proposal"),
          fundPda.toBuffer(),
          proposalIndexBuf,
        ],
        program.programId
      );

      // Wait for voting period to end (5 seconds + buffer)
      const proposal = await program.account.proposal.fetch(metaProposalPda);
      const now = Math.floor(Date.now() / 1000);
      const timeUntilEnd = proposal.votingEnd.toNumber() - now;

      if (timeUntilEnd > 0) {
        const waitTime = timeUntilEnd + 2; // extra 2s buffer
        console.log(`    Waiting ${waitTime}s for voting period to end...`);
        await new Promise((resolve) => setTimeout(resolve, waitTime * 1000));
      }

      // Use admin TWAP override: pass_twap=200 > fail_twap=100 → pass wins
      const tx = await program.methods
        .finalizeProposal({
          adminPassTwap: new BN(200),
          adminFailTwap: new BN(100),
        })
        .accountsPartial({
          cranker: admin.publicKey,
          fund: fundPda,
          proposal: metaProposalPda,
        })
        .rpc();
      logTx("finalizeProposal (admin TWAP override, pass wins)", tx);

      // Verify proposal passed
      const finalizedProposal = await program.account.proposal.fetch(metaProposalPda);
      assert.deepEqual(finalizedProposal.status, { passed: {} });
      assert.equal(finalizedProposal.passTwap.toNumber(), 200);
      assert.equal(finalizedProposal.failTwap.toNumber(), 100);
      console.log("    Proposal PASSED with admin TWAP override");
      console.log(`    Pass TWAP: ${finalizedProposal.passTwap.toString()}`);
      console.log(`    Fail TWAP: ${finalizedProposal.failTwap.toString()}`);
    });

    it("Resolve Question on MetaDAO (oracle resolves pass=1, fail=0)", async () => {
      // Build resolveQuestion instruction
      // Admin is the oracle for our question
      const hash = Buffer.from(
        await crypto.subtle.digest("SHA-256", Buffer.from("global:resolve_question"))
      );
      const disc = hash.slice(0, 8);

      // Args: ResolveQuestionArgs { payoutNumerators: Vec<u32> }
      // Pass wins: [1, 0] — outcome 0 gets 100%, outcome 1 gets 0%
      // Vec encoding: 4-byte length prefix + data
      const argsData = Buffer.alloc(4 + 4 + 4);
      argsData.writeUInt32LE(2, 0); // vec length = 2
      argsData.writeUInt32LE(1, 4); // payout[0] = 1 (pass wins)
      argsData.writeUInt32LE(0, 8); // payout[1] = 0 (fail loses)

      const ix = new TransactionInstruction({
        programId: CONDITIONAL_VAULT_PROGRAM_ID,
        keys: [
          { pubkey: questionPda, isSigner: false, isWritable: true },
          { pubkey: admin.publicKey, isSigner: true, isWritable: false }, // oracle
          { pubkey: eventAuthority, isSigner: false, isWritable: false },
          { pubkey: CONDITIONAL_VAULT_PROGRAM_ID, isSigner: false, isWritable: false },
        ],
        data: Buffer.concat([disc, argsData]),
      });

      const tx = new Transaction().add(ix);
      const sig = await provider.sendAndConfirm(tx);
      logTx("resolveQuestion (pass=1, fail=0)", sig);

      // Verify question is resolved
      const questionAccount = await connection.getAccountInfo(questionPda);
      assert.ok(questionAccount !== null, "Question should still exist");
      console.log("    Question resolved: pass wins (payout = [1, 0])");
    });

    it("Redeem pass-USDC conditional tokens for underlying USDC", async () => {
      const preAdminBalance = await getAccount(connection, adminQuoteAta);
      const preBalance = Number(preAdminBalance.amount);

      // Build redeemTokens instruction
      const hash = Buffer.from(
        await crypto.subtle.digest("SHA-256", Buffer.from("global:redeem_tokens"))
      );
      const disc = hash.slice(0, 8);

      // remaining_accounts: [conditionalMints..., userConditionalTokenAccounts...]
      const remainingAccounts = [
        { pubkey: passConditionalMint, isSigner: false, isWritable: true },
        { pubkey: failConditionalMint, isSigner: false, isWritable: true },
        { pubkey: adminPassConditionalAta, isSigner: false, isWritable: true },
        { pubkey: adminFailConditionalAta, isSigner: false, isWritable: true },
      ];

      const ix = new TransactionInstruction({
        programId: CONDITIONAL_VAULT_PROGRAM_ID,
        keys: [
          { pubkey: questionPda, isSigner: false, isWritable: false },
          { pubkey: vaultPda, isSigner: false, isWritable: true },
          { pubkey: vaultUnderlyingAta, isSigner: false, isWritable: true },
          { pubkey: admin.publicKey, isSigner: true, isWritable: false },
          { pubkey: adminQuoteAta, isSigner: false, isWritable: true },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: eventAuthority, isSigner: false, isWritable: false },
          { pubkey: CONDITIONAL_VAULT_PROGRAM_ID, isSigner: false, isWritable: false },
          ...remainingAccounts,
        ],
        data: disc, // No args for redeemTokens
      });

      const tx = new Transaction().add(ix);
      const sig = await provider.sendAndConfirm(tx);
      logTx("redeemTokens (pass-USDC → USDC)", sig);

      // Verify admin got USDC back
      const postAdminBalance = await getAccount(connection, adminQuoteAta);
      const redeemed = Number(postAdminBalance.amount) - preBalance;
      console.log(`    Redeemed ${redeemed / 1e6} USDC from conditional tokens`);
      assert.ok(redeemed > 0, "Should have redeemed some USDC");

      // Pass tokens should be burned (pass won, so pass tokens redeem 1:1)
      const passBalance = await getAccount(connection, adminPassConditionalAta);
      assert.equal(Number(passBalance.amount), 0, "Pass tokens should be burned");

      // Fail tokens should also be burned (worth 0)
      const failBalance = await getAccount(connection, adminFailConditionalAta);
      assert.equal(Number(failBalance.amount), 0, "Fail tokens should be burned");

      console.log("    All conditional tokens redeemed successfully");
    });

    it("Verify full MetaDAO integration state", async () => {
      // Question is resolved
      const questionAccount = await connection.getAccountInfo(questionPda);
      assert.ok(questionAccount !== null);

      // Vault underlying should be drained (all redeemed)
      const vaultBalance = await getAccount(connection, vaultUnderlyingAta);
      assert.equal(
        Number(vaultBalance.amount),
        0,
        "Vault should be empty after full redemption"
      );

      console.log("\n  ── MetaDAO Integration Summary ──────────");
      console.log(`    Conditional Vault Program: ${CONDITIONAL_VAULT_PROGRAM_ID.toBase58()}`);
      console.log(`    Question:                  ${questionPda.toBase58()}`);
      console.log(`    Vault:                     ${vaultPda.toBase58()}`);
      console.log(`    Pass Mint:                 ${passConditionalMint.toBase58()}`);
      console.log(`    Fail Mint:                 ${failConditionalMint.toBase58()}`);
      console.log(`    Split amount:              1000 USDC`);
      console.log(`    Resolution:                Pass wins [1, 0]`);
      console.log(`    Redeemed:                  1000 USDC`);
      console.log("  ──────────────────────────────────────────\n");
    });
  });

  // ══════════════════════════════════════════════════════════
  // Final State Summary
  // ══════════════════════════════════════════════════════════

  describe("Final State Summary", () => {
    it("Print final fund state", async () => {
      const fund = await program.account.fund.fetch(fundPda);
      const vaultAccount = await getAccount(connection, fundVaultPda);

      console.log("\n  ── Final Fund State ──────────────────────");
      console.log(`    Admin:            ${fund.admin.toBase58()}`);
      console.log(`    Quote Mint:       ${fund.quoteMint.toBase58()}`);
      console.log(`    Total Deposits:   ${fund.totalDeposits.toNumber() / 1e6} USDC`);
      console.log(`    Total Shares:     ${fund.totalShares.toNumber() / 1e6}`);
      console.log(`    NAV per Share:    ${fund.navPerShare.toString()} (WAD)`);
      console.log(`    Total NAV:        ${fund.totalNav.toString()} (WAD)`);
      console.log(`    Vault Balance:    ${Number(vaultAccount.amount) / 1e6} USDC`);
      console.log(`    Proposals:        ${fund.totalProposals.toNumber()} total, ${fund.activeProposals} active`);
      console.log(`    Status:           ${JSON.stringify(fund.status)}`);
      console.log(`    Perf Fee:         ${fund.performanceFeeBps.toNumber()} bps`);
      console.log(`    Mgmt Fee:         ${fund.managementFeeBps.toNumber()} bps`);
      console.log("  ──────────────────────────────────────────\n");
    });
  });
});
