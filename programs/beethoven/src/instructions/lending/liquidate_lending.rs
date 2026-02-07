use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::LendingLiquidated;
use crate::math::interest::accrue_interest;
use crate::math::fixed_point::{wad_mul, bps_mul};
use crate::math::liquidation::compute_lending_health_factor;
use crate::math::oracle::get_price;
use crate::state::{Exchange, LendingPool, LendingPosition, VaultState};

use anchor_spl::token::{TokenAccount, Token};

#[derive(Accounts)]
pub struct LiquidateLending<'info> {
    #[account(mut)]
    pub liquidator: Signer<'info>,

    #[account(
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
    )]
    pub exchange: Box<Account<'info, Exchange>>,

    #[account(
        mut,
        seeds = [LENDING_POOL_SEED, &lending_pool.pool_index.to_le_bytes()],
        bump = lending_pool.bump,
    )]
    pub lending_pool: Box<Account<'info, LendingPool>>,

    #[account(
        mut,
        seeds = [LENDING_POSITION_SEED, borrower.key().as_ref(), lending_pool.key().as_ref()],
        bump = lending_position.bump,
        constraint = lending_position.owner == borrower.key() @ ErrorCode::Unauthorized,
    )]
    pub lending_position: Box<Account<'info, LendingPosition>>,

    /// CHECK: The borrower being liquidated
    pub borrower: UncheckedAccount<'info>,

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

    /// Liquidator repays debt from this account
    #[account(mut)]
    pub liquidator_repay_token_account: Account<'info, TokenAccount>,

    /// Liquidator receives collateral to this account
    #[account(mut)]
    pub liquidator_receive_token_account: Account<'info, TokenAccount>,

    /// CHECK: Pyth oracle price feed
    pub oracle: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<LiquidateLending>, repay_amount: u64) -> Result<()> {
    require!(repay_amount > 0, ErrorCode::InvalidAmount);

    let pool = &mut ctx.accounts.lending_pool;
    let clock = Clock::get()?;

    accrue_interest(pool, clock.unix_timestamp)?;

    let position = &ctx.accounts.lending_position;
    let oracle_price = get_price(&ctx.accounts.oracle.to_account_info(), &clock)?;

    // Check position is liquidatable
    let collateral_value = (position.deposited_amount as u128)
        .checked_mul(oracle_price.price as u128)
        .ok_or(ErrorCode::MathOverflow)?;
    let weighted_collateral = wad_mul(collateral_value, pool.collateral_factor)?;
    let borrow_value = (position.borrowed_amount as u128)
        .checked_mul(oracle_price.price as u128)
        .ok_or(ErrorCode::MathOverflow)?;

    let health = compute_lending_health_factor(weighted_collateral, borrow_value)?;
    require!(
        health < LENDING_LIQUIDATION_THRESHOLD,
        ErrorCode::LendingNotLiquidatable
    );

    // Cap repay at 50% of debt
    let max_repay = bps_mul(position.borrowed_amount, MAX_LIQUIDATION_FRACTION_BPS)?;
    let actual_repay = repay_amount.min(max_repay);

    // Calculate collateral to seize (repay_amount + bonus)
    let bonus = bps_mul(actual_repay, ctx.accounts.exchange.liquidation_bonus_bps)?;
    let collateral_to_seize = actual_repay
        .checked_add(bonus)
        .ok_or(ErrorCode::MathOverflow)?
        .min(position.deposited_amount);

    // Liquidator repays debt: transfer tokens to vault
    anchor_spl::token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.liquidator_repay_token_account.to_account_info(),
                to: ctx.accounts.vault_token_account.to_account_info(),
                authority: ctx.accounts.liquidator.to_account_info(),
            },
        ),
        actual_repay,
    )?;

    // Transfer seized collateral from vault to liquidator
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
                to: ctx.accounts.liquidator_receive_token_account.to_account_info(),
                authority: ctx.accounts.vault_state.to_account_info(),
            },
            signer_seeds,
        ),
        collateral_to_seize,
    )?;

    // Update position
    let position = &mut ctx.accounts.lending_position;
    position.borrowed_amount = position
        .borrowed_amount
        .saturating_sub(actual_repay);
    position.deposited_amount = position
        .deposited_amount
        .saturating_sub(collateral_to_seize);
    position.last_updated = clock.unix_timestamp;

    // Update pool
    pool.total_borrows = pool.total_borrows.saturating_sub(actual_repay);
    pool.total_deposits = pool.total_deposits.saturating_sub(collateral_to_seize);

    emit!(LendingLiquidated {
        user: ctx.accounts.borrower.key(),
        pool: ctx.accounts.lending_pool.key(),
        liquidator: ctx.accounts.liquidator.key(),
        debt_repaid: actual_repay,
        collateral_seized: collateral_to_seize,
        liquidation_bonus: bonus,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
