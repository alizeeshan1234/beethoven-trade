use anchor_lang::prelude::*;
use crate::constants::WAD;
use crate::error::ErrorCode;

/// Multiply two WAD values: (a * b) / WAD
pub fn wad_mul(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(WAD)
        .ok_or(ErrorCode::DivisionByZero.into())
}

/// Divide two WAD values: (a * WAD) / b
pub fn wad_div(a: u128, b: u128) -> Result<u128> {
    if b == 0 {
        return Err(ErrorCode::DivisionByZero.into());
    }
    a.checked_mul(WAD)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(b)
        .ok_or(ErrorCode::DivisionByZero.into())
}

/// Convert a u64 value to WAD precision
pub fn to_wad(value: u64) -> Result<u128> {
    (value as u128)
        .checked_mul(WAD)
        .ok_or(ErrorCode::MathOverflow.into())
}

/// Convert WAD value back to u64
pub fn from_wad(value: u128) -> Result<u64> {
    let result = value.checked_div(WAD).ok_or(ErrorCode::DivisionByZero)?;
    u64::try_from(result).map_err(|_| ErrorCode::MathOverflow.into())
}

/// Multiply a value by basis points: (value * bps) / 10_000
pub fn bps_mul(value: u64, bps: u64) -> Result<u64> {
    (value as u128)
        .checked_mul(bps as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(10_000)
        .ok_or(ErrorCode::DivisionByZero)?
        .try_into()
        .map_err(|_| ErrorCode::MathOverflow.into())
}

/// Signed WAD multiply for funding calculations
pub fn wad_mul_signed(a: i128, b: i128) -> Result<i128> {
    a.checked_mul(b)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(WAD as i128)
        .ok_or(ErrorCode::DivisionByZero.into())
}
