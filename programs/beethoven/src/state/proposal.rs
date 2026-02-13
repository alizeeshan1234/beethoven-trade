use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionType {
    Swap,
    OpenPerp,
    ClosePerp,
    DepositLending,
    WithdrawLending,
    Borrow,
    Repay,
    UpdateParam,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProposalStatus {
    Active,
    Passed,
    Failed,
    Executed,
    Expired,
}

#[account]
pub struct Proposal {
    pub fund: Pubkey,
    pub proposer: Pubkey,
    pub bump: u8,
    pub proposal_index: u64,

    // Action
    pub action_type: ActionType,
    pub action_data: [u8; 256], // Borsh-serialized action params

    // MetaDAO conditional markets
    pub pass_market: Pubkey,
    pub fail_market: Pubkey,

    // TWAP readings
    pub pass_twap: u128,
    pub fail_twap: u128,

    // Status
    pub status: ProposalStatus,

    // Timing
    pub voting_start: i64,
    pub voting_end: i64,
    pub execution_deadline: i64,
    pub executed_at: i64,

    // Reserved for future use
    pub _reserved: [u8; 128],
}

impl Proposal {
    pub const LEN: usize = 8  // discriminator
        + 32  // fund
        + 32  // proposer
        + 1   // bump
        + 8   // proposal_index
        + 1   // action_type (enum)
        + 256 // action_data
        + 32  // pass_market
        + 32  // fail_market
        + 16  // pass_twap
        + 16  // fail_twap
        + 1   // status (enum)
        + 8   // voting_start
        + 8   // voting_end
        + 8   // execution_deadline
        + 8   // executed_at
        + 128; // reserved
}

// Action data structs — Borsh-serialized into action_data field

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SwapActionData {
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub amount_in: u64,
    pub minimum_amount_out: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PerpActionData {
    pub market_index: u16,
    pub is_long: bool,
    pub size: u64,
    pub collateral: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct LendingActionData {
    pub pool_index: u16,
    pub amount: u64,
}
