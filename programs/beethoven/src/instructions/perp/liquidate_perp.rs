use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::PerpLiquidated;
use crate::math::fixed_point::bps_mul;
use crate::math::funding::compute_position_funding;
use crate::math::liquidation::{compute_pnl, compute_perp_health_factor, is_perp_liquidatable};
use crate::math::oracle::get_price;
use crate::state::{Exchange, PerpMarket, PerpPosition, UserAccount, VaultState};
use crate::state::perp_position::PositionSide;

use anchor_spl::token::{TokenAccount, Token};

#[derive(Accounts)]
pub struct LiquidatePerp<'info> {
    #[account(mut)]
    pub liquidator: Signer<'info>,

    #[account(
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
    )]
    pub exchange: Box<Account<'info, Exchange>>,

    #[account(
        mut,
        seeds = [USER_ACCOUNT_SEED, position_owner.key().as_ref()],
        bump = user_account.bump,
    )]
    pub user_account: Account<'info, UserAccount>,

    #[account(
        mut,
        seeds = [PERP_MARKET_SEED, &perp_market.market_index.to_le_bytes()],
        bump = perp_market.bump,
    )]
    pub perp_market: Box<Account<'info, PerpMarket>>,

    #[account(
        mut,
        constraint = perp_position.owner == position_owner.key() @ ErrorCode::Unauthorized,
        constraint = perp_position.market == perp_market.key() @ ErrorCode::PositionNotFound,
        close = position_owner,
    )]
    pub perp_position: Box<Account<'info, PerpPosition>>,

    /// CHECK: The owner of the position being liquidated
    #[account(mut)]
    pub position_owner: UncheckedAccount<'info>,

    /// CHECK: Pyth oracle price feed
    #[account(
        constraint = oracle.key() == perp_market.oracle @ ErrorCode::OracleAccountMismatch,
    )]
    pub oracle: UncheckedAccount<'info>,

    #[account(
        seeds = [VAULT_SEED, perp_market.quote_mint.as_ref()],
        bump = vault_state.bump,
    )]
    pub vault_state: Box<Account<'info, VaultState>>,

    #[account(
        mut,
        constraint = vault_token_account.key() == vault_state.token_account @ ErrorCode::InvalidParameter,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    /// Liquidator receives bonus to this account
    #[account(mut)]
    pub liquidator_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<LiquidatePerp>) -> Result<()> {
    let position = &ctx.accounts.perp_position;
    let market = &ctx.accounts.perp_market;
    let exchange = &ctx.accounts.exchange;
    let clock = Clock::get()?;

    // Get current price
    let oracle_price = get_price(&ctx.accounts.oracle.to_account_info(), &clock)?;

    // Compute PnL
    let pnl = compute_pnl(
        &position.side,
        position.size,
        position.entry_price,
        oracle_price.price,
    )?;

    // Compute funding
    let funding_payment = compute_position_funding(
        position.size,
        position.side == PositionSide::Long,
        market.cumulative_funding_long,
        market.cumulative_funding_short,
        position.cumulative_funding_snapshot,
    )?;

    // Check health factor
    let health = compute_perp_health_factor(
        position.collateral,
        pnl,
        funding_payment,
        position.size,
        oracle_price.price,
    )?;
    require!(is_perp_liquidatable(health), ErrorCode::NotLiquidatable);

    // Calculate liquidation bonus
    let bonus = bps_mul(position.collateral, exchange.liquidation_bonus_bps)?;

    // Remaining collateral after losses
    let remaining = (position.collateral as i64)
        .checked_add(pnl)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_sub(funding_payment)
        .ok_or(ErrorCode::MathOverflow)?;

    let liquidator_reward = if remaining > 0 {
        bonus.min(remaining as u64)
    } else {
        0
    };

    // Transfer reward to liquidator
    if liquidator_reward > 0 {
        let mint_key = market.quote_mint;
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
                    to: ctx.accounts.liquidator_token_account.to_account_info(),
                    authority: ctx.accounts.vault_state.to_account_info(),
                },
                signer_seeds,
            ),
            liquidator_reward,
        )?;
    }

    // Update market OI
    let market = &mut ctx.accounts.perp_market;
    match position.side {
        PositionSide::Long => {
            market.long_open_interest = market
                .long_open_interest
                .saturating_sub(position.size);
        }
        PositionSide::Short => {
            market.short_open_interest = market
                .short_open_interest
                .saturating_sub(position.size);
        }
    }

    // Update user account
    let user = &mut ctx.accounts.user_account;
    user.open_perp_positions = user.open_perp_positions.saturating_sub(1);
    user.total_pnl = user
        .total_pnl
        .checked_add(pnl)
        .ok_or(ErrorCode::MathOverflow)?;

    emit!(PerpLiquidated {
        user: ctx.accounts.position_owner.key(),
        market: ctx.accounts.perp_market.key(),
        liquidator: ctx.accounts.liquidator.key(),
        size: position.size,
        collateral_seized: position.collateral,
        liquidation_bonus: liquidator_reward,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
