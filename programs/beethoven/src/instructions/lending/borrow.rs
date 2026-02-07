use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::BorrowExecuted;
use crate::math::interest::accrue_interest;
use crate::math::fixed_point::wad_mul;
use crate::math::liquidation::compute_lending_health_factor;
use crate::math::oracle::get_price;
use crate::state::{Exchange, LendingPool, LendingPosition, VaultState};

use anchor_spl::token::{TokenAccount, Token};

#[derive(Accounts)]
pub struct Borrow<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
        constraint = !exchange.lending_paused @ ErrorCode::ExchangePaused,
    )]
    pub exchange: Box<Account<'info, Exchange>>,

    #[account(
        mut,
        seeds = [LENDING_POOL_SEED, &lending_pool.pool_index.to_le_bytes()],
        bump = lending_pool.bump,
        constraint = !lending_pool.paused @ ErrorCode::ExchangePaused,
    )]
    pub lending_pool: Box<Account<'info, LendingPool>>,

    #[account(
        mut,
        seeds = [LENDING_POSITION_SEED, owner.key().as_ref(), lending_pool.key().as_ref()],
        bump = lending_position.bump,
        constraint = lending_position.owner == owner.key() @ ErrorCode::Unauthorized,
    )]
    pub lending_position: Box<Account<'info, LendingPosition>>,

    #[account(
        seeds = [VAULT_SEED, lending_pool.mint.as_ref()],
        bump = vault_state.bump,
    )]
    pub vault_state: Box<Account<'info, VaultState>>,

    #[account(
        mut,
        constraint = vault_token_account.key() == lending_pool.vault @ ErrorCode::InvalidParameter,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_token_account.owner == owner.key() @ ErrorCode::Unauthorized,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    /// CHECK: Pyth oracle price feed
    pub oracle: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<Borrow>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let pool = &mut ctx.accounts.lending_pool;
    let clock = Clock::get()?;

    accrue_interest(pool, clock.unix_timestamp)?;

    // Check pool has enough liquidity
    let available = pool
        .total_deposits
        .checked_sub(pool.total_borrows)
        .ok_or(ErrorCode::InsufficientPoolLiquidity)?;
    require!(amount <= available, ErrorCode::InsufficientPoolLiquidity);

    // Check borrow limit
    let new_total_borrows = pool
        .total_borrows
        .checked_add(amount)
        .ok_or(ErrorCode::MathOverflow)?;
    if pool.borrow_limit > 0 {
        require!(
            new_total_borrows <= pool.borrow_limit,
            ErrorCode::InsufficientPoolLiquidity
        );
    }

    // Check health factor after borrow
    let position = &ctx.accounts.lending_position;
    let oracle_price = get_price(&ctx.accounts.oracle.to_account_info(), &clock)?;

    let collateral_value = (position.deposited_amount as u128)
        .checked_mul(oracle_price.price as u128)
        .ok_or(ErrorCode::MathOverflow)?;
    let weighted_collateral = wad_mul(collateral_value, pool.collateral_factor)?;

    let new_borrow_amount = position
        .borrowed_amount
        .checked_add(amount)
        .ok_or(ErrorCode::MathOverflow)?;
    let borrow_value = (new_borrow_amount as u128)
        .checked_mul(oracle_price.price as u128)
        .ok_or(ErrorCode::MathOverflow)?;

    let health = compute_lending_health_factor(weighted_collateral, borrow_value)?;
    require!(
        health >= LENDING_LIQUIDATION_THRESHOLD,
        ErrorCode::InsufficientCollateralValue
    );

    // Transfer tokens from vault to user
    let mint_key = ctx.accounts.vault_state.mint;
    let seeds = &[
        VAULT_SEED,
        mint_key.as_ref(),
        &[ctx.accounts.vault_state.bump],
    ];
    let signer_seeds = &[&seeds[..]];

    anchor_spl::token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.vault_token_account.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.vault_state.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
    )?;

    // Update state
    let position = &mut ctx.accounts.lending_position;
    position.borrowed_amount = new_borrow_amount;
    position.cumulative_borrow_rate_snapshot = pool.cumulative_borrow_rate;
    position.last_updated = clock.unix_timestamp;

    pool.total_borrows = new_total_borrows;

    emit!(BorrowExecuted {
        user: ctx.accounts.owner.key(),
        pool: ctx.accounts.lending_pool.key(),
        amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
