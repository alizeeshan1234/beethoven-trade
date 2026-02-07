use anchor_lang::prelude::*;
use crate::constants::{WAD, MAX_FUNDING_RATE};
use crate::error::ErrorCode;
use crate::math::fixed_point::wad_mul_signed;
use crate::state::PerpMarket;

/// Calculate the funding rate based on OI imbalance.
/// Positive rate = longs pay shorts; negative = shorts pay longs.
/// Rate = (long_oi - short_oi) / (long_oi + short_oi) * base_factor
pub fn calculate_funding_rate(market: &PerpMarket) -> Result<i128> {
    let total_oi = market
        .long_open_interest
        .checked_add(market.short_open_interest)
        .ok_or(ErrorCode::MathOverflow)?;

    if total_oi == 0 {
        return Ok(0);
    }

    let imbalance = (market.long_open_interest as i128)
        .checked_sub(market.short_open_interest as i128)
        .ok_or(ErrorCode::MathOverflow)?;

    // funding_rate = imbalance * WAD / total_oi
    let rate = imbalance
        .checked_mul(WAD as i128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(total_oi as i128)
        .ok_or(ErrorCode::DivisionByZero)?;

    // Clamp to max funding rate
    let max = MAX_FUNDING_RATE as i128;
    Ok(rate.clamp(-max, max))
}

/// Compute funding payment for a position.
/// funding_payment = size * (current_cumulative - snapshot_cumulative)
/// Positive payment means the position pays; negative means it receives.
pub fn compute_position_funding(
    size: u64,
    is_long: bool,
    cumulative_funding_long: i128,
    cumulative_funding_short: i128,
    position_snapshot: i128,
) -> Result<i64> {
    let current_cumulative = if is_long {
        cumulative_funding_long
    } else {
        cumulative_funding_short
    };

    let delta = current_cumulative
        .checked_sub(position_snapshot)
        .ok_or(ErrorCode::MathOverflow)?;

    let payment = wad_mul_signed(size as i128, delta)?;

    i64::try_from(payment).map_err(|_| ErrorCode::MathOverflow.into())
}
