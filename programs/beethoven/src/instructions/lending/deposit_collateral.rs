use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::CollateralDeposited;
use crate::math::interest::accrue_interest;
use crate::state::{Exchange, LendingPool, LendingPosition, UserAccount};
use anchor_spl::token::{TokenAccount, Token};
#[derive(Accounts)]
pub struct DepositCollateral<'info> {
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
        seeds = [USER_ACCOUNT_SEED, owner.key().as_ref()],
        bump = user_account.bump,
        constraint = user_account.owner == owner.key() @ ErrorCode::Unauthorized,
    )]
    pub user_account: Box<Account<'info, UserAccount>>,

    #[account(
        mut,
        seeds = [LENDING_POOL_SEED, &lending_pool.pool_index.to_le_bytes()],
        bump = lending_pool.bump,
        constraint = !lending_pool.paused @ ErrorCode::ExchangePaused,
    )]
    pub lending_pool: Box<Account<'info, LendingPool>>,

    #[account(
        init_if_needed,
        payer = owner,
        space = LendingPosition::LEN,
        seeds = [LENDING_POSITION_SEED, owner.key().as_ref(), lending_pool.key().as_ref()],
        bump,
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
        constraint = user_token_account.mint == lending_pool.mint @ ErrorCode::InvalidParameter,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<DepositCollateral>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    // Capture keys before mutable borrows
    let pool_key = ctx.accounts.lending_pool.key();
    let clock = Clock::get()?;

    let pool = &mut ctx.accounts.lending_pool;

    // Accrue interest before any state changes
    accrue_interest(pool, clock.unix_timestamp)?;

    // Check deposit limit
    let new_total = pool
        .total_deposits
        .checked_add(amount)
        .ok_or(ErrorCode::MathOverflow)?;
    if pool.deposit_limit > 0 {
        require!(new_total <= pool.deposit_limit, ErrorCode::InvalidAmount);
    }

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
        amount,
    )?;

    // Update pool state
    pool.total_deposits = new_total;

    let deposit_rate = pool.cumulative_deposit_rate;
    let borrow_rate = pool.cumulative_borrow_rate;

    // Update position
    let position = &mut ctx.accounts.lending_position;
    if position.owner == Pubkey::default() {
        // First-time init
        position.owner = ctx.accounts.owner.key();
        position.pool = pool_key;
        position.bump = ctx.bumps.lending_position;
        position.cumulative_deposit_rate_snapshot = deposit_rate;
        position.cumulative_borrow_rate_snapshot = borrow_rate;
        position._reserved = [0u8; 64];

        let user = &mut ctx.accounts.user_account;
        user.open_lending_positions = user
            .open_lending_positions
            .checked_add(1)
            .ok_or(ErrorCode::MaxLendingPositionsReached)?;
    }

    position.deposited_amount = position
        .deposited_amount
        .checked_add(amount)
        .ok_or(ErrorCode::MathOverflow)?;
    position.last_updated = clock.unix_timestamp;

    // Update user activity
    ctx.accounts.user_account.last_activity = clock.unix_timestamp;

    emit!(CollateralDeposited {
        user: ctx.accounts.owner.key(),
        pool: ctx.accounts.lending_pool.key(),
        amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
