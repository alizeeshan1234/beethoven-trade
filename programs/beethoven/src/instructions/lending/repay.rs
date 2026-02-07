use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::RepayExecuted;
use crate::math::interest::{accrue_interest, get_borrow_balance};
use crate::state::{LendingPool, LendingPosition};

use anchor_spl::token::{TokenAccount, Token};

#[derive(Accounts)]
pub struct Repay<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [LENDING_POOL_SEED, &lending_pool.pool_index.to_le_bytes()],
        bump = lending_pool.bump,
    )]
    pub lending_pool: Account<'info, LendingPool>,

    #[account(
        mut,
        seeds = [LENDING_POSITION_SEED, owner.key().as_ref(), lending_pool.key().as_ref()],
        bump = lending_position.bump,
        constraint = lending_position.owner == owner.key() @ ErrorCode::Unauthorized,
    )]
    pub lending_position: Account<'info, LendingPosition>,

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

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<Repay>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let pool = &mut ctx.accounts.lending_pool;
    let clock = Clock::get()?;

    accrue_interest(pool, clock.unix_timestamp)?;

    let position = &mut ctx.accounts.lending_position;

    // Calculate current borrow balance with accrued interest
    let current_debt = get_borrow_balance(
        position.borrowed_amount,
        position.cumulative_borrow_rate_snapshot,
        pool.cumulative_borrow_rate,
    )?;

    // Cap repay at current debt
    let repay_amount = amount.min(current_debt);
    require!(repay_amount > 0, ErrorCode::InvalidAmount);

    // Transfer tokens from user to vault
    anchor_spl::token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.vault_token_account.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ),
        repay_amount,
    )?;

    // Update position
    if repay_amount >= current_debt {
        position.borrowed_amount = 0;
    } else {
        position.borrowed_amount = current_debt
            .checked_sub(repay_amount)
            .ok_or(ErrorCode::MathUnderflow)?;
    }
    position.cumulative_borrow_rate_snapshot = pool.cumulative_borrow_rate;
    position.last_updated = clock.unix_timestamp;

    // Update pool
    pool.total_borrows = pool
        .total_borrows
        .saturating_sub(repay_amount);

    emit!(RepayExecuted {
        user: ctx.accounts.owner.key(),
        pool: ctx.accounts.lending_pool.key(),
        amount: repay_amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
