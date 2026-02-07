use anchor_lang::prelude::*;
use anchor_spl::token::Mint as AnchorMint;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::LendingPoolCreated;
use crate::state::{Exchange, LendingPool, VaultState};

use anchor_spl::token::{TokenAccount, Token};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateLendingPoolParams {
    pub pool_index: u16,
    pub optimal_utilization: u128,
    pub base_rate: u128,
    pub slope1: u128,
    pub slope2: u128,
    pub collateral_factor: u128,
    pub deposit_limit: u64,
    pub borrow_limit: u64,
}

#[derive(Accounts)]
#[instruction(params: CreateLendingPoolParams)]
pub struct CreateLendingPool<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
        constraint = exchange.admin == admin.key() @ ErrorCode::Unauthorized,
    )]
    pub exchange: Account<'info, Exchange>,

    #[account(
        init,
        payer = admin,
        space = LendingPool::LEN,
        seeds = [LENDING_POOL_SEED, &params.pool_index.to_le_bytes()],
        bump,
    )]
    pub lending_pool: Account<'info, LendingPool>,

    pub mint: Account<'info, AnchorMint>,

    /// CHECK: Pyth oracle price feed
    pub oracle: UncheckedAccount<'info>,

    #[account(
        init,
        payer = admin,
        space = VaultState::LEN,
        seeds = [VAULT_SEED, mint.key().as_ref()],
        bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        init,
        payer = admin,
        token::mint = mint,
        token::authority = vault_state,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<CreateLendingPool>, params: CreateLendingPoolParams) -> Result<()> {
    // Validate collateral factor is <= 1.0 (WAD)
    require!(
        params.collateral_factor <= WAD,
        ErrorCode::InvalidCollateralFactor
    );

    // Capture keys before mutable borrows
    let pool_key = ctx.accounts.lending_pool.key();
    let exchange_key = ctx.accounts.exchange.key();
    let mint_key = ctx.accounts.mint.key();
    let vault_token_key = ctx.accounts.vault_token_account.key();
    let oracle_key = ctx.accounts.oracle.key();
    let now = Clock::get()?.unix_timestamp;

    let pool = &mut ctx.accounts.lending_pool;
    pool.exchange = exchange_key;
    pool.bump = ctx.bumps.lending_pool;
    pool.mint = mint_key;
    pool.vault = vault_token_key;
    pool.pool_index = params.pool_index;
    pool.oracle = oracle_key;
    pool.optimal_utilization = params.optimal_utilization;
    pool.base_rate = params.base_rate;
    pool.slope1 = params.slope1;
    pool.slope2 = params.slope2;
    pool.collateral_factor = params.collateral_factor;
    pool.total_deposits = 0;
    pool.total_borrows = 0;
    pool.cumulative_deposit_rate = WAD; // Start at 1.0
    pool.cumulative_borrow_rate = WAD;  // Start at 1.0
    pool.last_update_timestamp = now;
    pool.deposit_limit = params.deposit_limit;
    pool.borrow_limit = params.borrow_limit;
    pool.paused = false;
    pool._reserved = [0u8; 128];

    let vault_state = &mut ctx.accounts.vault_state;
    vault_state.exchange = exchange_key;
    vault_state.mint = mint_key;
    vault_state.token_account = vault_token_key;
    vault_state.bump = ctx.bumps.vault_state;
    vault_state.collected_fees = 0;
    vault_state.insurance_balance = 0;
    vault_state._reserved = [0u8; 64];

    let exchange = &mut ctx.accounts.exchange;
    exchange.total_lending_pools = exchange
        .total_lending_pools
        .checked_add(1)
        .ok_or(ErrorCode::MathOverflow)?;

    emit!(LendingPoolCreated {
        pool: pool_key,
        mint: mint_key,
        timestamp: now,
    });

    Ok(())
}
