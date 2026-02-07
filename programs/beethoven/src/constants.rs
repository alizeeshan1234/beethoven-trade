use anchor_lang::prelude::*;

// PDA Seeds
#[constant]
pub const EXCHANGE_SEED: &[u8] = b"exchange";
#[constant]
pub const USER_ACCOUNT_SEED: &[u8] = b"user_account";
#[constant]
pub const PERP_MARKET_SEED: &[u8] = b"perp_market";
#[constant]
pub const PERP_POSITION_SEED: &[u8] = b"perp_position";
#[constant]
pub const LENDING_POOL_SEED: &[u8] = b"lending_pool";
#[constant]
pub const LENDING_POSITION_SEED: &[u8] = b"lending_position";
#[constant]
pub const VAULT_SEED: &[u8] = b"vault";

// WAD precision (1e18) for fixed-point math
pub const WAD: u128 = 1_000_000_000_000_000_000;

// Price precision (1e6) matching USDC decimals
pub const PRICE_PRECISION: u64 = 1_000_000;

// Basis points denominator
pub const BPS_DENOMINATOR: u64 = 10_000;

// Fee limits (in basis points)
pub const MAX_SWAP_FEE_BPS: u64 = 100; // 1%
pub const MAX_PERP_FEE_BPS: u64 = 50; // 0.5%
pub const MAX_LENDING_FEE_BPS: u64 = 200; // 2%

// Leverage limits
pub const MIN_LEVERAGE: u64 = 1;
pub const MAX_LEVERAGE: u64 = 50;
pub const DEFAULT_MAX_LEVERAGE: u64 = 20;

// Liquidation thresholds
pub const PERP_LIQUIDATION_THRESHOLD: u64 = 500; // 5% margin ratio -> liquidation
pub const LENDING_LIQUIDATION_THRESHOLD: u128 = WAD; // health factor < 1.0
pub const LIQUIDATION_BONUS_BPS: u64 = 500; // 5% bonus to liquidator
pub const MAX_LIQUIDATION_FRACTION_BPS: u64 = 5_000; // Can liquidate up to 50% per tx

// Interest rate model defaults (in WAD units)
pub const DEFAULT_OPTIMAL_UTILIZATION: u128 = 800_000_000_000_000_000; // 80%
pub const DEFAULT_BASE_RATE: u128 = 20_000_000_000_000_000; // 2%
pub const DEFAULT_SLOPE1: u128 = 40_000_000_000_000_000; // 4%
pub const DEFAULT_SLOPE2: u128 = 750_000_000_000_000_000; // 75%

// Funding rate
pub const FUNDING_INTERVAL: i64 = 3600; // 1 hour in seconds
pub const MAX_FUNDING_RATE: u128 = 10_000_000_000_000_000; // 1% per interval

// Oracle
pub const MAX_ORACLE_STALENESS: u64 = 60; // 60 seconds
pub const PYTH_PRICE_EXPO_ADJUSTMENT: i32 = -8; // Pyth typically uses exponent -8

// Position limits
pub const MAX_PERP_POSITIONS: u8 = 10;
pub const MAX_LENDING_POSITIONS: u8 = 10;
