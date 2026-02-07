use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionSide {
    #[default]
    Long,
    Short,
}

#[account]
pub struct PerpPosition {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub bump: u8,

    // Position details
    pub side: PositionSide,
    pub size: u64,          // Position size in base units
    pub collateral: u64,    // Collateral in quote units
    pub entry_price: u64,   // Entry price (PRICE_PRECISION)
    pub leverage: u64,      // Effective leverage (1x = 1)

    // Funding snapshot (WAD precision)
    pub cumulative_funding_snapshot: i128,

    // Computed at open
    pub liquidation_price: u64,

    // PnL tracking
    pub realized_pnl: i64,
    pub unrealized_pnl: i64,

    // Timestamps
    pub opened_at: i64,
    pub last_updated: i64,

    // Reserved
    pub _reserved: [u8; 64],
}

impl PerpPosition {
    pub const LEN: usize = 8  // discriminator
        + 32  // owner
        + 32  // market
        + 1   // bump
        + 1   // side
        + 8   // size
        + 8   // collateral
        + 8   // entry_price
        + 8   // leverage
        + 16  // cumulative_funding_snapshot
        + 8   // liquidation_price
        + 8   // realized_pnl
        + 8   // unrealized_pnl
        + 8   // opened_at
        + 8   // last_updated
        + 64; // reserved
}
