use anchor_lang::prelude::*;

#[account]
pub struct PerpMarket {
    pub exchange: Pubkey,
    pub bump: u8,

    // Market pair
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub market_index: u16,

    // Oracle
    pub oracle: Pubkey,

    // Leverage limits
    pub max_leverage: u64,
    pub min_position_size: u64,

    // Open interest tracking (in base units)
    pub long_open_interest: u64,
    pub short_open_interest: u64,
    pub max_open_interest: u64,

    // Funding rate (WAD precision, signed)
    pub funding_rate: i128,
    pub cumulative_funding_long: i128,
    pub cumulative_funding_short: i128,
    pub last_funding_update: i64,

    // Status
    pub paused: bool,

    // Reserved for future use
    pub _reserved: [u8; 128],
}

impl PerpMarket {
    pub const LEN: usize = 8   // discriminator
        + 32  // exchange
        + 1   // bump
        + 32  // base_mint
        + 32  // quote_mint
        + 2   // market_index
        + 32  // oracle
        + 8   // max_leverage
        + 8   // min_position_size
        + 8 * 3 // OI fields
        + 16 * 3 // funding fields (i128)
        + 8   // last_funding_update
        + 1   // paused
        + 128; // reserved
}
