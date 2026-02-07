use anchor_lang::prelude::*;

#[account]
pub struct LendingPool {
    pub exchange: Pubkey,
    pub bump: u8,

    // Token
    pub mint: Pubkey,
    pub vault: Pubkey, // Token account holding pool assets
    pub pool_index: u16,

    // Oracle
    pub oracle: Pubkey,

    // Interest rate model params (WAD precision)
    pub optimal_utilization: u128,
    pub base_rate: u128,
    pub slope1: u128,
    pub slope2: u128,

    // Collateral factor (WAD precision, e.g., 0.8e18 = 80% LTV)
    pub collateral_factor: u128,

    // Pool state
    pub total_deposits: u64,
    pub total_borrows: u64,

    // Cumulative rate accumulators (WAD precision)
    pub cumulative_deposit_rate: u128,
    pub cumulative_borrow_rate: u128,
    pub last_update_timestamp: i64,

    // Limits
    pub deposit_limit: u64,
    pub borrow_limit: u64,

    // Status
    pub paused: bool,

    // Reserved
    pub _reserved: [u8; 128],
}

impl LendingPool {
    pub const LEN: usize = 8  // discriminator
        + 32  // exchange
        + 1   // bump
        + 32  // mint
        + 32  // vault
        + 2   // pool_index
        + 32  // oracle
        + 16 * 4 // interest rate params
        + 16  // collateral_factor
        + 8   // total_deposits
        + 8   // total_borrows
        + 16 * 2 // cumulative rates
        + 8   // last_update_timestamp
        + 8   // deposit_limit
        + 8   // borrow_limit
        + 1   // paused
        + 128; // reserved
}
