use anchor_lang::prelude::*;

#[account]
pub struct LendingPosition {
    pub owner: Pubkey,
    pub pool: Pubkey,
    pub bump: u8,

    // Deposit (in pool token units)
    pub deposited_amount: u64,

    // Borrow (in pool token units)
    pub borrowed_amount: u64,

    // Cumulative rate snapshots (WAD precision)
    pub cumulative_deposit_rate_snapshot: u128,
    pub cumulative_borrow_rate_snapshot: u128,

    // Timestamps
    pub last_updated: i64,

    // Reserved
    pub _reserved: [u8; 64],
}

impl LendingPosition {
    pub const LEN: usize = 8  // discriminator
        + 32  // owner
        + 32  // pool
        + 1   // bump
        + 8   // deposited_amount
        + 8   // borrowed_amount
        + 16  // cumulative_deposit_rate_snapshot
        + 16  // cumulative_borrow_rate_snapshot
        + 8   // last_updated
        + 64; // reserved
}
