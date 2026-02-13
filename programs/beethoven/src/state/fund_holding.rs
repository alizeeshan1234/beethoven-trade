use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HoldingType {
    Spot,
    PerpLong,
    PerpShort,
    LendingDeposit,
    LendingBorrow,
}

#[account]
pub struct FundHolding {
    pub fund: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,       // Token account for this holding
    pub bump: u8,

    // Oracle for pricing
    pub oracle: Pubkey,

    // Position data
    pub amount: u64,
    pub value_usd: u128,     // WAD precision

    // Type of holding
    pub holding_type: HoldingType,

    // Related position account (e.g., PerpPosition or LendingPosition)
    pub related_position: Pubkey,

    // Index within the fund's holding list
    pub holding_index: u8,

    // Timestamps
    pub last_updated: i64,

    // Reserved for future use
    pub _reserved: [u8; 64],
}

impl FundHolding {
    pub const LEN: usize = 8  // discriminator
        + 32  // fund
        + 32  // mint
        + 32  // vault
        + 1   // bump
        + 32  // oracle
        + 8   // amount
        + 16  // value_usd
        + 1   // holding_type (enum)
        + 32  // related_position
        + 1   // holding_index
        + 8   // last_updated
        + 64; // reserved
}
