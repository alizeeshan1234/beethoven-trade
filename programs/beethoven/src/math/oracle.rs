use anchor_lang::prelude::*;
use crate::constants::MAX_ORACLE_STALENESS;
use crate::error::ErrorCode;

/// Parsed price from oracle feed
pub struct OraclePrice {
    pub price: u64,     // in PRICE_PRECISION (1e6)
    pub confidence: u64,
    pub timestamp: i64,
}

/// Parse a Pyth price feed from an AccountInfo.
/// Pyth PriceUpdateV2 layout (simplified):
///   - bytes [0..8]: discriminator
///   - bytes [8..12]: verification level (u32)
///   - price feed embedded at fixed offset
///
/// We parse the price feed data which contains:
///   - price_message.price (i64), exponent (i32), conf (u64), publish_time (i64)
pub fn get_price(oracle_account: &AccountInfo, clock: &Clock) -> Result<OraclePrice> {
    let data = oracle_account.try_borrow_data()?;

    // Minimum size check for Pyth PriceUpdateV2
    require!(data.len() >= 112, ErrorCode::OraclePriceInvalid);

    // Parse price message fields from the PriceUpdateV2 account
    // Layout after discriminator(8) + write_authority(32) + verification_level(1):
    //   feed_id: [u8; 32] at offset 41
    //   price: i64 at offset 73
    //   conf: u64 at offset 81
    //   exponent: i32 at offset 89
    //   publish_time: i64 at offset 93
    let price_raw = i64::from_le_bytes(
        data[73..81].try_into().map_err(|_| ErrorCode::OraclePriceInvalid)?
    );
    let conf_raw = u64::from_le_bytes(
        data[81..89].try_into().map_err(|_| ErrorCode::OraclePriceInvalid)?
    );
    let exponent = i32::from_le_bytes(
        data[89..93].try_into().map_err(|_| ErrorCode::OraclePriceInvalid)?
    );
    let publish_time = i64::from_le_bytes(
        data[93..101].try_into().map_err(|_| ErrorCode::OraclePriceInvalid)?
    );

    // Validate price is positive
    require!(price_raw > 0, ErrorCode::OraclePriceInvalid);

    // Check staleness
    let age = clock
        .unix_timestamp
        .checked_sub(publish_time)
        .ok_or(ErrorCode::MathOverflow)?;
    require!(age <= MAX_ORACLE_STALENESS as i64, ErrorCode::OraclePriceStale);

    // Convert price to PRICE_PRECISION (1e6)
    let price = normalize_price(price_raw as u64, exponent)?;
    let confidence = normalize_price(conf_raw, exponent)?;

    Ok(OraclePrice {
        price,
        confidence,
        timestamp: publish_time,
    })
}

/// Normalize a Pyth price with exponent to PRICE_PRECISION (1e6)
fn normalize_price(raw_price: u64, exponent: i32) -> Result<u64> {
    // Pyth exponent is typically negative (e.g., -8)
    // target = 6 decimals (PRICE_PRECISION = 1e6)
    let target_exp: i32 = 6;
    let shift = target_exp + exponent; // e.g., 6 + (-8) = -2

    if shift >= 0 {
        raw_price
            .checked_mul(10u64.pow(shift as u32))
            .ok_or(ErrorCode::MathOverflow.into())
    } else {
        let divisor = 10u64.pow((-shift) as u32);
        Ok(raw_price / divisor)
    }
}
