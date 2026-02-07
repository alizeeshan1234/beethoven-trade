use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::FundingRateUpdated;
use crate::math::funding::calculate_funding_rate;
use crate::state::PerpMarket;

#[derive(Accounts)]
pub struct UpdateFundingRate<'info> {
    /// Anyone can crank the funding rate (permissionless)
    pub cranker: Signer<'info>,

    #[account(
        mut,
        seeds = [PERP_MARKET_SEED, &perp_market.market_index.to_le_bytes()],
        bump = perp_market.bump,
    )]
    pub perp_market: Account<'info, PerpMarket>,
}

pub fn handler(ctx: Context<UpdateFundingRate>) -> Result<()> {
    let market_key = ctx.accounts.perp_market.key();
    let market = &mut ctx.accounts.perp_market;
    let clock = Clock::get()?;

    // Check that enough time has passed since last update
    let elapsed = clock
        .unix_timestamp
        .checked_sub(market.last_funding_update)
        .ok_or(ErrorCode::MathOverflow)?;

    require!(
        elapsed >= FUNDING_INTERVAL,
        ErrorCode::FundingIntervalNotElapsed
    );

    // Calculate new funding rate
    let new_rate = calculate_funding_rate(market)?;
    market.funding_rate = new_rate;

    // Update cumulative funding rates
    // Longs pay: cumulative_long += rate (positive rate means longs pay)
    // Shorts receive: cumulative_short -= rate
    market.cumulative_funding_long = market
        .cumulative_funding_long
        .checked_add(new_rate)
        .ok_or(ErrorCode::MathOverflow)?;

    market.cumulative_funding_short = market
        .cumulative_funding_short
        .checked_sub(new_rate)
        .ok_or(ErrorCode::MathOverflow)?;

    market.last_funding_update = clock.unix_timestamp;

    emit!(FundingRateUpdated {
        market: market_key,
        funding_rate: new_rate,
        cumulative_funding_long: market.cumulative_funding_long,
        cumulative_funding_short: market.cumulative_funding_short,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
