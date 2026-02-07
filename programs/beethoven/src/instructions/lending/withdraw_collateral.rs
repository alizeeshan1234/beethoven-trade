use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::CollateralWithdrawn;
use crate::math::interest::accrue_interest;
use crate::math::fixed_point::wad_mul;
use crate::math::liquidation::compute_lending_health_factor;
use crate::math::oracle::get_price;
use crate::state::{Exchange, LendingPool, LendingPosition, VaultState};

#[derive(Accounts)]
pub struct WithdrawCollateral<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

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
    pub vault_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(
        mut,
        constraint = user_token_account.owner == owner.key() @ ErrorCode::Unauthorized,
    )]
    pub user_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    /// CHECK: Pyth oracle price feed
    pub oracle: UncheckedAccount<'info>,

    pub token_program: Program<'info, anchor_spl::token::Token>,
}

pub fn handler(ctx: Context<WithdrawCollateral>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let pool = &mut ctx.accounts.lending_pool;
    let clock = Clock::get()?;

    accrue_interest(pool, clock.unix_timestamp)?;

    let position = &mut ctx.accounts.lending_position;
    require!(
        position.deposited_amount >= amount,
        ErrorCode::InsufficientCollateralValue
    );

    // If user has borrows, check that withdrawal won't make position unhealthy
    if position.borrowed_amount > 0 {
        let oracle_price = get_price(&ctx.accounts.oracle.to_account_info(), &clock)?;
        let remaining_deposit = position
            .deposited_amount
            .checked_sub(amount)
            .ok_or(ErrorCode::MathUnderflow)?;

        let collateral_value = (remaining_deposit as u128)
            .checked_mul(oracle_price.price as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        let weighted_collateral = wad_mul(collateral_value, pool.collateral_factor)?;

        let borrow_value = (position.borrowed_amount as u128)
            .checked_mul(oracle_price.price as u128)
            .ok_or(ErrorCode::MathOverflow)?;

        let health = compute_lending_health_factor(weighted_collateral, borrow_value)?;
        require!(
            health >= LENDING_LIQUIDATION_THRESHOLD,
            ErrorCode::WithdrawalWouldLiquidate
        );
    }

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
    position.deposited_amount = position
        .deposited_amount
        .checked_sub(amount)
        .ok_or(ErrorCode::MathUnderflow)?;
    position.last_updated = clock.unix_timestamp;

    pool.total_deposits = pool
        .total_deposits
        .checked_sub(amount)
        .ok_or(ErrorCode::MathUnderflow)?;

    emit!(CollateralWithdrawn {
        user: ctx.accounts.owner.key(),
        pool: ctx.accounts.lending_pool.key(),
        amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
