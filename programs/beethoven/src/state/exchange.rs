use anchor_lang::prelude::*;

#[account]
pub struct Exchange {
    pub admin: Pubkey,
    pub bump: u8,

    // Fee settings (in basis points)
    pub swap_fee_bps: u64,
    pub perp_open_fee_bps: u64,
    pub perp_close_fee_bps: u64,
    pub lending_fee_bps: u64,

    // Leverage
    pub max_leverage: u64,

    // Liquidation
    pub liquidation_bonus_bps: u64,
    pub max_liquidation_fraction_bps: u64,

    // Pause flags
    pub swap_paused: bool,
    pub perp_paused: bool,
    pub lending_paused: bool,

    // Counters
    pub total_perp_markets: u64,
    pub total_lending_pools: u64,
    pub total_users: u64,

    // Reserved for future use
    pub _reserved: [u8; 128],
}

impl Exchange {
    pub const LEN: usize = 8 // discriminator
        + 32  // admin
        + 1   // bump
        + 8 * 4 // fees
        + 8   // max_leverage
        + 8 * 2 // liquidation params
        + 1 * 3 // pause flags
        + 8 * 3 // counters
        + 128; // reserved
}
