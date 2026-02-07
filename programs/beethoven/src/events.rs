use anchor_lang::prelude::*;

// Swap events
#[event]
pub struct SwapExecuted {
    pub user: Pubkey,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee: u64,
    pub protocol: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct LiquidityAdded {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub protocol: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct LiquidityRemoved {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub protocol: Pubkey,
    pub timestamp: i64,
}

// Perp events
#[event]
pub struct PerpPositionOpened {
    pub user: Pubkey,
    pub market: Pubkey,
    pub is_long: bool,
    pub size: u64,
    pub collateral: u64,
    pub entry_price: u64,
    pub leverage: u64,
    pub timestamp: i64,
}

#[event]
pub struct PerpPositionClosed {
    pub user: Pubkey,
    pub market: Pubkey,
    pub is_long: bool,
    pub size_closed: u64,
    pub exit_price: u64,
    pub pnl: i64,
    pub fee: u64,
    pub timestamp: i64,
}

#[event]
pub struct PerpLiquidated {
    pub user: Pubkey,
    pub market: Pubkey,
    pub liquidator: Pubkey,
    pub size: u64,
    pub collateral_seized: u64,
    pub liquidation_bonus: u64,
    pub timestamp: i64,
}

#[event]
pub struct FundingRateUpdated {
    pub market: Pubkey,
    pub funding_rate: i128,
    pub cumulative_funding_long: i128,
    pub cumulative_funding_short: i128,
    pub timestamp: i64,
}

// Lending events
#[event]
pub struct CollateralDeposited {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct CollateralWithdrawn {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct BorrowExecuted {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct RepayExecuted {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct LendingLiquidated {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub liquidator: Pubkey,
    pub debt_repaid: u64,
    pub collateral_seized: u64,
    pub liquidation_bonus: u64,
    pub timestamp: i64,
}

// Admin events
#[event]
pub struct ExchangeInitialized {
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct PerpMarketCreated {
    pub market: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct LendingPoolCreated {
    pub pool: Pubkey,
    pub mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct FeesCollected {
    pub vault: Pubkey,
    pub amount: u64,
    pub recipient: Pubkey,
    pub timestamp: i64,
}
