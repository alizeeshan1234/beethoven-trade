use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::PerpPositionClosed;
use crate::math::fixed_point::bps_mul;
use crate::math::funding::compute_position_funding;
use crate::math::liquidation::compute_pnl;
use crate::math::oracle::get_price;
use crate::state::{Exchange, PerpMarket, PerpPosition, UserAccount, VaultState};
use crate::state::perp_position::PositionSide;

#[derive(Accounts)]
pub struct ClosePosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
    )]
    pub exchange: Box<Account<'info, Exchange>>,

    #[account(
        mut,
        seeds = [USER_ACCOUNT_SEED, owner.key().as_ref()],
        bump = user_account.bump,
        constraint = user_account.owner == owner.key() @ ErrorCode::Unauthorized,
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
        constraint = perp_position.owner == owner.key() @ ErrorCode::Unauthorized,
        constraint = perp_position.market == perp_market.key() @ ErrorCode::PositionNotFound,
        close = owner,
    )]
    pub perp_position: Box<Account<'info, PerpPosition>>,

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
    pub vault_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(
        mut,
        constraint = user_token_account.owner == owner.key() @ ErrorCode::Unauthorized,
    )]
    pub user_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    pub token_program: Program<'info, anchor_spl::token::Token>,
}

pub fn handler(ctx: Context<ClosePosition>) -> Result<()> {
    let position = &ctx.accounts.perp_position;
    let market = &ctx.accounts.perp_market;
    let exchange = &ctx.accounts.exchange;
    let clock = Clock::get()?;

    // Get current price
    let oracle_price = get_price(&ctx.accounts.oracle.to_account_info(), &clock)?;

    // Calculate PnL
    let pnl = compute_pnl(
        &position.side,
        position.size,
        position.entry_price,
        oracle_price.price,
    )?;

    // Calculate funding payment
    let funding_payment = compute_position_funding(
        position.size,
        position.side == PositionSide::Long,
        market.cumulative_funding_long,
        market.cumulative_funding_short,
        position.cumulative_funding_snapshot,
    )?;

    // Calculate fee
    let notional = (position.size as u128)
        .checked_mul(oracle_price.price as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(PRICE_PRECISION as u128)
        .ok_or(ErrorCode::DivisionByZero)?;
    let fee = bps_mul(notional as u64, exchange.perp_close_fee_bps)?;

    // Net payout = collateral + pnl - funding - fee
    let payout = (position.collateral as i64)
        .checked_add(pnl)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_sub(funding_payment)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_sub(fee as i64)
        .ok_or(ErrorCode::MathOverflow)?;

    // Transfer payout if positive
    if payout > 0 {
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
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.vault_state.to_account_info(),
                },
                signer_seeds,
            ),
            payout as u64,
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
    user.total_fees_paid = user
        .total_fees_paid
        .checked_add(fee)
        .ok_or(ErrorCode::MathOverflow)?;
    user.last_activity = clock.unix_timestamp;

    emit!(PerpPositionClosed {
        user: ctx.accounts.owner.key(),
        market: ctx.accounts.perp_market.key(),
        is_long: position.side == PositionSide::Long,
        size_closed: position.size,
        exit_price: oracle_price.price,
        pnl,
        fee,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
