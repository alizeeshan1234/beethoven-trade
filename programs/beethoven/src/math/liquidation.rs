use anchor_lang::prelude::*;
use crate::constants::{PRICE_PRECISION, BPS_DENOMINATOR, PERP_LIQUIDATION_THRESHOLD};
use crate::error::ErrorCode;
use crate::math::fixed_point::wad_div;
use crate::state::perp_position::PositionSide;

/// Compute PnL for a perpetual position.
/// Long PnL = size * (current_price - entry_price) / PRICE_PRECISION
/// Short PnL = size * (entry_price - current_price) / PRICE_PRECISION
pub fn compute_pnl(
    side: &PositionSide,
    size: u64,
    entry_price: u64,
    current_price: u64,
) -> Result<i64> {
    match side {
        PositionSide::Long => {
            let diff = current_price as i64 - entry_price as i64;
            Ok((size as i128)
                .checked_mul(diff as i128)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(PRICE_PRECISION as i128)
                .ok_or(ErrorCode::DivisionByZero)? as i64)
        }
        PositionSide::Short => {
            let diff = entry_price as i64 - current_price as i64;
            Ok((size as i128)
                .checked_mul(diff as i128)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(PRICE_PRECISION as i128)
                .ok_or(ErrorCode::DivisionByZero)? as i64)
        }
    }
}

/// Compute health factor for a perp position.
/// health = (collateral + pnl - abs(funding)) / (size * current_price / PRICE_PRECISION)
/// Returns value in BPS (e.g., 1000 = 10%).
pub fn compute_perp_health_factor(
    collateral: u64,
    pnl: i64,
    funding_payment: i64,
    size: u64,
    current_price: u64,
) -> Result<u64> {
    let effective_collateral = (collateral as i64)
        .checked_add(pnl)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_sub(funding_payment)
        .ok_or(ErrorCode::MathOverflow)?;

    if effective_collateral <= 0 {
        return Ok(0);
    }

    let notional = (size as u128)
        .checked_mul(current_price as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(PRICE_PRECISION as u128)
        .ok_or(ErrorCode::DivisionByZero)?;

    if notional == 0 {
        return Ok(BPS_DENOMINATOR); // No exposure
    }

    // health_bps = effective_collateral * BPS_DENOMINATOR / notional
    let health = (effective_collateral as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(notional)
        .ok_or(ErrorCode::DivisionByZero)?;

    Ok(health as u64)
}

/// Check if a perp position is liquidatable.
/// Liquidatable if health_factor < PERP_LIQUIDATION_THRESHOLD (5%).
pub fn is_perp_liquidatable(health_factor: u64) -> bool {
    health_factor < PERP_LIQUIDATION_THRESHOLD
}

/// Compute liquidation price for a perp position.
/// Long liq price = entry_price - (collateral * PRICE_PRECISION / size) * (1 - maintenance_margin)
/// Short liq price = entry_price + (collateral * PRICE_PRECISION / size) * (1 - maintenance_margin)
pub fn compute_liquidation_price(
    side: &PositionSide,
    entry_price: u64,
    collateral: u64,
    size: u64,
) -> Result<u64> {
    if size == 0 {
        return Ok(0);
    }

    // margin_per_unit = collateral * PRICE_PRECISION / size
    let margin_per_unit = (collateral as u128)
        .checked_mul(PRICE_PRECISION as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(size as u128)
        .ok_or(ErrorCode::DivisionByZero)?;

    // Adjust by maintenance margin (PERP_LIQUIDATION_THRESHOLD = 500 bps = 5%)
    // Effective margin = margin_per_unit * (BPS - threshold) / BPS
    let effective_margin = margin_per_unit
        .checked_mul((BPS_DENOMINATOR - PERP_LIQUIDATION_THRESHOLD) as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(ErrorCode::DivisionByZero)?;

    match side {
        PositionSide::Long => {
            let liq_price = (entry_price as u128)
                .checked_sub(effective_margin)
                .unwrap_or(0);
            Ok(liq_price as u64)
        }
        PositionSide::Short => {
            let liq_price = (entry_price as u128)
                .checked_add(effective_margin)
                .ok_or(ErrorCode::MathOverflow)?;
            Ok(liq_price as u64)
        }
    }
}

/// Compute lending health factor.
/// health_factor = sum(deposit_value * collateral_factor) / total_borrow_value
/// Returns WAD precision (1e18 = 1.0).
pub fn compute_lending_health_factor(
    weighted_collateral_value: u128, // WAD precision
    total_borrow_value: u128,        // WAD precision
) -> Result<u128> {
    if total_borrow_value == 0 {
        return Ok(u128::MAX); // No borrows = infinite health
    }
    wad_div(weighted_collateral_value, total_borrow_value)
}
