use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    // General (6000-6009)
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Math underflow")]
    MathUnderflow,
    #[msg("Division by zero")]
    DivisionByZero,
    #[msg("Invalid amount: must be greater than zero")]
    InvalidAmount,
    #[msg("Unauthorized: signer is not the admin")]
    Unauthorized,
    #[msg("Exchange is paused")]
    ExchangePaused,
    #[msg("Invalid parameter")]
    InvalidParameter,
    #[msg("Account already initialized")]
    AlreadyInitialized,

    // Oracle (6010-6019)
    #[msg("Oracle price is stale")]
    OraclePriceStale,
    #[msg("Oracle price is invalid or negative")]
    OraclePriceInvalid,
    #[msg("Oracle confidence interval too wide")]
    OracleConfidenceTooWide,
    #[msg("Oracle account mismatch")]
    OracleAccountMismatch,

    // Swap (6020-6029)
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    #[msg("Unsupported swap protocol")]
    UnsupportedProtocol,
    #[msg("Swap returned zero output")]
    SwapOutputZero,
    #[msg("Insufficient balance for swap")]
    InsufficientSwapBalance,

    // Perp (6030-6049)
    #[msg("Leverage exceeds maximum allowed")]
    ExcessiveLeverage,
    #[msg("Position size too small")]
    PositionTooSmall,
    #[msg("Position not found")]
    PositionNotFound,
    #[msg("Open interest limit exceeded")]
    OpenInterestLimitExceeded,
    #[msg("Position is not liquidatable")]
    NotLiquidatable,
    #[msg("Funding interval not elapsed")]
    FundingIntervalNotElapsed,
    #[msg("Invalid position side")]
    InvalidPositionSide,
    #[msg("Close amount exceeds position size")]
    CloseAmountExceedsPosition,
    #[msg("Insufficient collateral for position")]
    InsufficientCollateral,
    #[msg("Maximum perp positions reached")]
    MaxPerpPositionsReached,

    // Lending (6050-6069)
    #[msg("Insufficient collateral value")]
    InsufficientCollateralValue,
    #[msg("Health factor below minimum")]
    HealthFactorBelowMinimum,
    #[msg("Borrow amount exceeds pool availability")]
    InsufficientPoolLiquidity,
    #[msg("Repay amount exceeds debt")]
    RepayExceedsDebt,
    #[msg("Withdrawal would make position unhealthy")]
    WithdrawalWouldLiquidate,
    #[msg("Lending position not liquidatable")]
    LendingNotLiquidatable,
    #[msg("Maximum lending positions reached")]
    MaxLendingPositionsReached,
    #[msg("Collateral factor out of range")]
    InvalidCollateralFactor,

    // Admin (6070-6079)
    #[msg("Fee exceeds maximum allowed")]
    FeeExceedsMaximum,
    #[msg("Leverage setting out of bounds")]
    LeverageOutOfBounds,
    #[msg("Insufficient vault balance for withdrawal")]
    InsufficientVaultBalance,

    // Fund (6080-6099)
    #[msg("Fund is paused")]
    FundPaused,
    #[msg("Insufficient shares for operation")]
    InsufficientShares,
    #[msg("Maximum active proposals reached")]
    MaxActiveProposals,
    #[msg("Maximum fund holdings reached")]
    MaxFundHoldings,
    #[msg("Proposal is not in active status")]
    ProposalNotActive,
    #[msg("Proposal did not pass")]
    ProposalNotPassed,
    #[msg("Proposal has expired past execution deadline")]
    ProposalExpired,
    #[msg("Voting period has not ended")]
    VotingPeriodNotEnded,
    #[msg("Invalid action data for proposal")]
    InvalidActionData,
    #[msg("NAV data is stale, update required")]
    NavStale,
    #[msg("Fund is winding down, only withdrawals allowed")]
    FundWindingDown,
    #[msg("Insufficient fund liquidity for withdrawal")]
    InsufficientFundLiquidity,
}
