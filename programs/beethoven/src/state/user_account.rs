use anchor_lang::prelude::*;

#[account]
pub struct UserAccount {
    pub owner: Pubkey,
    pub bump: u8,

    // Position counts
    pub open_perp_positions: u8,
    pub open_lending_positions: u8,

    // Cumulative stats
    pub total_trades: u64,
    pub total_pnl: i64,
    pub total_volume: u64,
    pub total_fees_paid: u64,

    // Referral
    pub referrer: Pubkey,

    // Timestamps
    pub created_at: i64,
    pub last_activity: i64,

    // Reserved for future use
    pub _reserved: [u8; 64],
}

impl UserAccount {
    pub const LEN: usize = 8 // discriminator
        + 32  // owner
        + 1   // bump
        + 1   // open_perp_positions
        + 1   // open_lending_positions
        + 8   // total_trades
        + 8   // total_pnl (i64)
        + 8   // total_volume
        + 8   // total_fees_paid
        + 32  // referrer
        + 8   // created_at
        + 8   // last_activity
        + 64; // reserved
}
