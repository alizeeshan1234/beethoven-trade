use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::NavUpdated;
use crate::math::nav::calculate_total_nav;
use crate::state::Fund;

#[derive(Accounts)]
pub struct UpdateFundNav<'info> {
    /// Permissionless cranker
    pub cranker: Signer<'info>,

    #[account(
        mut,
        seeds = [FUND_SEED],
        bump = fund.bump,
    )]
    pub fund: Account<'info, Fund>,

    /// Fund's USDC vault
    #[account(
        seeds = [FUND_VAULT_SEED],
        bump,
        constraint = fund_vault.key() == fund.fund_vault @ ErrorCode::InvalidParameter,
    )]
    pub fund_vault: Account<'info, TokenAccount>,
    // remaining_accounts: [FundHolding, Oracle] pairs
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, UpdateFundNav<'info>>,
) -> Result<()> {
    let clock = Clock::get()?;
    let vault_balance = ctx.accounts.fund_vault.amount;
    let total_shares = ctx.accounts.fund.total_shares;

    // Calculate NAV from vault balance + holdings
    let nav_result = calculate_total_nav(
        vault_balance,
        ctx.remaining_accounts,
        total_shares,
        &clock,
    )?;

    // Update fund state
    let fund = &mut ctx.accounts.fund;
    fund.total_nav = nav_result.total_nav_wad;
    fund.nav_per_share = nav_result.nav_per_share_wad;
    fund.last_nav_update = clock.unix_timestamp;

    // Update high water mark if NAV per share exceeded it
    if nav_result.nav_per_share_wad > fund.high_water_mark {
        fund.high_water_mark = nav_result.nav_per_share_wad;
    }

    emit!(NavUpdated {
        fund: ctx.accounts.fund.key(),
        total_nav: nav_result.total_nav_wad,
        nav_per_share: nav_result.nav_per_share_wad,
        total_shares,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
