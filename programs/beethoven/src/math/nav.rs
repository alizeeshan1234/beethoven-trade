use anchor_lang::prelude::*;
use crate::constants::WAD;
use crate::error::ErrorCode;
use crate::math::fixed_point::{to_wad, wad_div, wad_mul};
use crate::math::oracle::get_price;
use crate::state::fund_holding::{FundHolding, HoldingType};

pub struct NavResult {
    pub total_nav_wad: u128,
    pub nav_per_share_wad: u128,
}

/// Calculate total NAV from vault USDC balance + all FundHolding values.
///
/// remaining_accounts must contain pairs of [FundHolding, Oracle] accounts.
/// Spot/PerpLong/LendingDeposit add to NAV, PerpShort/LendingBorrow subtract.
pub fn calculate_total_nav<'info>(
    vault_balance: u64,
    remaining_accounts: &[AccountInfo<'info>],
    total_shares: u64,
    clock: &Clock,
) -> Result<NavResult> {
    // Start with vault USDC balance as base NAV
    let mut total_nav_wad = to_wad(vault_balance)?;

    // Process holding/oracle pairs
    let num_pairs = remaining_accounts.len() / 2;
    for i in 0..num_pairs {
        let holding_info = &remaining_accounts[i * 2];
        let oracle_info = &remaining_accounts[i * 2 + 1];

        // Deserialize the FundHolding account
        let holding_data = holding_info.try_borrow_data()?;
        // Skip the 8-byte discriminator
        let holding: FundHolding = FundHolding::try_deserialize(
            &mut &holding_data[..],
        ).map_err(|_| ErrorCode::InvalidActionData)?;

        // Get oracle price for this holding
        let oracle_price = get_price(oracle_info, clock)?;

        // Calculate holding value: amount * price / PRICE_PRECISION, then to WAD
        let value_wad = if holding.amount > 0 {
            let amount_wad = to_wad(holding.amount)?;
            let price_wad = to_wad(oracle_price.price)?;
            let precision_wad = to_wad(crate::constants::PRICE_PRECISION)?;
            wad_mul(amount_wad, wad_div(price_wad, precision_wad)?)?
        } else {
            0u128
        };

        // Add or subtract based on holding type
        match holding.holding_type {
            HoldingType::Spot | HoldingType::PerpLong | HoldingType::LendingDeposit => {
                total_nav_wad = total_nav_wad
                    .checked_add(value_wad)
                    .ok_or(ErrorCode::MathOverflow)?;
            }
            HoldingType::PerpShort | HoldingType::LendingBorrow => {
                total_nav_wad = total_nav_wad
                    .checked_sub(value_wad)
                    .ok_or(ErrorCode::MathUnderflow)?;
            }
        }
    }

    // Calculate NAV per share
    // total_nav_wad is already in WAD, total_shares is raw token count.
    // Dividing WAD-value by raw count yields WAD-precision per-share value.
    // e.g., 1.3e27 (WAD) / 1.3e9 (shares) = 1e18 = 1.0 WAD
    let nav_per_share_wad = if total_shares == 0 {
        WAD // 1.0 when no shares exist
    } else {
        total_nav_wad
            .checked_div(total_shares as u128)
            .ok_or(ErrorCode::DivisionByZero)?
    };

    Ok(NavResult {
        total_nav_wad,
        nav_per_share_wad,
    })
}
