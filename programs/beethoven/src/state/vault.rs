use anchor_lang::prelude::*;

#[account]
pub struct VaultState {
    pub exchange: Pubkey,
    pub mint: Pubkey,
    pub token_account: Pubkey,
    pub bump: u8,

    // Fee tracking
    pub collected_fees: u64,
    pub insurance_balance: u64,

    // Reserved
    pub _reserved: [u8; 64],
}

impl VaultState {
    pub const LEN: usize = 8  // discriminator
        + 32  // exchange
        + 32  // mint
        + 32  // token_account
        + 1   // bump
        + 8   // collected_fees
        + 8   // insurance_balance
        + 64; // reserved
}
