use anchor_lang::prelude::*;
use crate::constants::WAD;
use crate::error::ErrorCode;
use crate::math::fixed_point::{wad_mul, wad_div};
use crate::state::LendingPool;

/// Calculate the borrow rate using a dual-slope model (Aave-style).
/// - Below optimal utilization: base_rate + (utilization / optimal) * slope1
/// - Above optimal utilization: base_rate + slope1 + ((utilization - optimal) / (1 - optimal)) * slope2
pub fn calculate_borrow_rate(pool: &LendingPool) -> Result<u128> {
    if pool.total_deposits == 0 {
        return Ok(pool.base_rate);
    }

    let utilization = wad_div(pool.total_borrows as u128, pool.total_deposits as u128)?;

    if utilization <= pool.optimal_utilization {
        // Below kink: base_rate + (utilization / optimal_utilization) * slope1
        let ratio = wad_div(utilization, pool.optimal_utilization)?;
        let variable = wad_mul(ratio, pool.slope1)?;
        pool.base_rate
            .checked_add(variable)
            .ok_or(ErrorCode::MathOverflow.into())
    } else {
        // Above kink: base_rate + slope1 + ((util - optimal) / (1 - optimal)) * slope2
        let excess = utilization
            .checked_sub(pool.optimal_utilization)
            .ok_or(ErrorCode::MathUnderflow)?;
        let remaining = WAD
            .checked_sub(pool.optimal_utilization)
            .ok_or(ErrorCode::MathUnderflow)?;
        let ratio = wad_div(excess, remaining)?;
        let variable = wad_mul(ratio, pool.slope2)?;
        pool.base_rate
            .checked_add(pool.slope1)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_add(variable)
            .ok_or(ErrorCode::MathOverflow.into())
    }
}

/// Accrue interest on a lending pool. Updates cumulative rate accumulators.
/// Called before any deposit/borrow/repay/withdraw operation.
pub fn accrue_interest(pool: &mut LendingPool, current_timestamp: i64) -> Result<()> {
    if pool.last_update_timestamp == 0 || current_timestamp <= pool.last_update_timestamp {
        pool.last_update_timestamp = current_timestamp;
        return Ok(());
    }

    let elapsed = (current_timestamp - pool.last_update_timestamp) as u128;
    let seconds_per_year: u128 = 365 * 24 * 3600;

    let borrow_rate = calculate_borrow_rate(pool)?;

    // Rate per elapsed time: borrow_rate * elapsed / seconds_per_year
    let period_rate = wad_mul(borrow_rate, wad_div(elapsed, seconds_per_year)?)?;

    // Update cumulative borrow rate: cumulative *= (1 + period_rate)
    let rate_factor = WAD.checked_add(period_rate).ok_or(ErrorCode::MathOverflow)?;
    pool.cumulative_borrow_rate = wad_mul(pool.cumulative_borrow_rate, rate_factor)?;

    // Calculate supply rate = borrow_rate * utilization
    if pool.total_deposits > 0 {
        let utilization = wad_div(pool.total_borrows as u128, pool.total_deposits as u128)?;
        let supply_period_rate = wad_mul(
            wad_mul(borrow_rate, utilization)?,
            wad_div(elapsed, seconds_per_year)?,
        )?;
        let supply_factor = WAD
            .checked_add(supply_period_rate)
            .ok_or(ErrorCode::MathOverflow)?;
        pool.cumulative_deposit_rate = wad_mul(pool.cumulative_deposit_rate, supply_factor)?;
    }

    pool.last_update_timestamp = current_timestamp;
    Ok(())
}

/// Get the current borrow balance for a position, accounting for accrued interest.
pub fn get_borrow_balance(
    borrowed_amount: u64,
    snapshot_rate: u128,
    current_rate: u128,
) -> Result<u64> {
    if borrowed_amount == 0 || snapshot_rate == 0 {
        return Ok(borrowed_amount);
    }
    let growth = wad_div(current_rate, snapshot_rate)?;
    let balance = wad_mul(borrowed_amount as u128, growth)?;
    u64::try_from(balance.checked_div(WAD).ok_or(ErrorCode::MathOverflow)?)
        .map_err(|_| ErrorCode::MathOverflow.into())
}

/// Get the current deposit balance for a position, accounting for accrued interest.
pub fn get_deposit_balance(
    deposited_amount: u64,
    snapshot_rate: u128,
    current_rate: u128,
) -> Result<u64> {
    if deposited_amount == 0 || snapshot_rate == 0 {
        return Ok(deposited_amount);
    }
    let growth = wad_div(current_rate, snapshot_rate)?;
    let balance = wad_mul(deposited_amount as u128, growth)?;
    u64::try_from(balance.checked_div(WAD).ok_or(ErrorCode::MathOverflow)?)
        .map_err(|_| ErrorCode::MathOverflow.into())
}
