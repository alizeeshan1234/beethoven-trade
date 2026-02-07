#![allow(ambiguous_glob_reexports)]

pub mod constants;
pub mod error;
pub mod events;
pub mod state;
pub mod math;
pub mod instructions;
pub mod adapters;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("CreqYFgN6nKYaBBuWvxs83PVJg6HSu1YYj3rriu7xNQu");

#[program]
pub mod beethoven {
    use super::*;

    // ── Admin ───────────────────────────────────────────────

    pub fn initialize_exchange(
        ctx: Context<InitializeExchange>,
        params: instructions::admin::initialize_exchange::InitializeExchangeParams,
    ) -> Result<()> {
        instructions::admin::initialize_exchange::handler(ctx, params)
    }

    pub fn create_perp_market(
        ctx: Context<CreatePerpMarket>,
        params: instructions::admin::create_perp_market::CreatePerpMarketParams,
    ) -> Result<()> {
        instructions::admin::create_perp_market::handler(ctx, params)
    }

    pub fn create_lending_pool(
        ctx: Context<CreateLendingPool>,
        params: instructions::admin::create_lending_pool::CreateLendingPoolParams,
    ) -> Result<()> {
        instructions::admin::create_lending_pool::handler(ctx, params)
    }

    pub fn update_funding_rate(ctx: Context<UpdateFundingRate>) -> Result<()> {
        instructions::admin::update_funding_rate::handler(ctx)
    }

    pub fn collect_fees(ctx: Context<CollectFees>, amount: u64) -> Result<()> {
        instructions::admin::collect_fees::handler(ctx, amount)
    }

    // ── User ────────────────────────────────────────────────

    pub fn create_user_account(
        ctx: Context<CreateUserAccount>,
        referrer: Option<Pubkey>,
    ) -> Result<()> {
        instructions::user::create_user_account::handler(ctx, referrer)
    }

    // ── Swap ────────────────────────────────────────────────

    pub fn execute_swap<'info>(
        ctx: Context<'_, '_, 'info, 'info, ExecuteSwap<'info>>,
        params: instructions::swap::execute_swap::ExecuteSwapParams,
    ) -> Result<()> {
        instructions::swap::execute_swap::handler(ctx, params)
    }

    pub fn add_liquidity<'info>(
        ctx: Context<'_, '_, 'info, 'info, AddLiquidity<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::swap::add_liquidity::handler(ctx, amount)
    }

    pub fn remove_liquidity<'info>(
        ctx: Context<'_, '_, 'info, 'info, RemoveLiquidity<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::swap::remove_liquidity::handler(ctx, amount)
    }

    // ── Perpetuals ──────────────────────────────────────────

    pub fn open_position(
        ctx: Context<OpenPosition>,
        params: instructions::perp::open_position::OpenPositionParams,
    ) -> Result<()> {
        instructions::perp::open_position::handler(ctx, params)
    }

    pub fn close_position(ctx: Context<ClosePosition>) -> Result<()> {
        instructions::perp::close_position::handler(ctx)
    }

    pub fn liquidate_perp(ctx: Context<LiquidatePerp>) -> Result<()> {
        instructions::perp::liquidate_perp::handler(ctx)
    }

    // ── Lending ─────────────────────────────────────────────

    pub fn deposit_collateral(ctx: Context<DepositCollateral>, amount: u64) -> Result<()> {
        instructions::lending::deposit_collateral::handler(ctx, amount)
    }

    pub fn withdraw_collateral(ctx: Context<WithdrawCollateral>, amount: u64) -> Result<()> {
        instructions::lending::withdraw_collateral::handler(ctx, amount)
    }

    pub fn borrow(ctx: Context<Borrow>, amount: u64) -> Result<()> {
        instructions::lending::borrow::handler(ctx, amount)
    }

    pub fn repay(ctx: Context<Repay>, amount: u64) -> Result<()> {
        instructions::lending::repay::handler(ctx, amount)
    }

    pub fn liquidate_lending(ctx: Context<LiquidateLending>, repay_amount: u64) -> Result<()> {
        instructions::lending::liquidate_lending::handler(ctx, repay_amount)
    }
}
