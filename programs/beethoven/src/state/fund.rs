use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FundStatus {
    Active,
    Paused,
    WindingDown,
}

#[account]
pub struct Fund {
    pub admin: Pubkey,
    pub bump: u8,

    // Token configuration
    pub quote_mint: Pubkey,  // USDC mint
    pub share_mint: Pubkey,  // SPL share token mint
    pub fund_vault: Pubkey,  // USDC vault (Fund PDA as authority)

    // Fund accounting
    pub total_deposits: u64,
    pub total_shares: u64,
    pub nav_per_share: u128,  // WAD precision
    pub total_nav: u128,      // WAD precision

    // Fee configuration (basis points)
    pub performance_fee_bps: u64,
    pub management_fee_bps: u64,
    pub fee_recipient: Pubkey,

    // Governance
    pub total_proposals: u64,
    pub active_proposals: u8,
    pub total_holdings: u8,

    // MetaDAO integration
    pub meta_dao_program: Pubkey,

    // Status
    pub status: FundStatus,

    // Timestamps
    pub created_at: i64,
    pub last_nav_update: i64,
    pub last_fee_collection: i64,

    // Performance tracking
    pub high_water_mark: u128, // WAD precision

    // Reserved for future use
    pub _reserved: [u8; 128],
}

impl Fund {
    pub const LEN: usize = 8  // discriminator
        + 32  // admin
        + 1   // bump
        + 32  // quote_mint
        + 32  // share_mint
        + 32  // fund_vault
        + 8   // total_deposits
        + 8   // total_shares
        + 16  // nav_per_share
        + 16  // total_nav
        + 8   // performance_fee_bps
        + 8   // management_fee_bps
        + 32  // fee_recipient
        + 8   // total_proposals
        + 1   // active_proposals
        + 1   // total_holdings
        + 32  // meta_dao_program
        + 1   // status (enum)
        + 8   // created_at
        + 8   // last_nav_update
        + 8   // last_fee_collection
        + 16  // high_water_mark
        + 128; // reserved
}
