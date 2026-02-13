use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, Transfer};
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::FundWithdrawal;
use crate::state::{Fund, FundStatus};

#[derive(Accounts)]
pub struct WithdrawFromFund<'info> {
    #[account(mut)]
    pub withdrawer: Signer<'info>,

    #[account(
        mut,
        seeds = [FUND_SEED],
        bump = fund.bump,
        constraint = fund.status != FundStatus::Paused @ ErrorCode::FundPaused,
    )]
    pub fund: Account<'info, Fund>,

    /// Withdrawer's share token account
    #[account(
        mut,
        constraint = user_share_account.mint == fund.share_mint @ ErrorCode::InvalidParameter,
        constraint = user_share_account.owner == withdrawer.key() @ ErrorCode::Unauthorized,
    )]
    pub user_share_account: Account<'info, TokenAccount>,

    /// Share token mint
    #[account(
        mut,
        seeds = [SHARE_MINT_SEED],
        bump,
        constraint = share_mint.key() == fund.share_mint @ ErrorCode::InvalidParameter,
    )]
    pub share_mint: Account<'info, Mint>,

    /// Withdrawer's USDC token account
    #[account(
        mut,
        constraint = user_token_account.mint == fund.quote_mint @ ErrorCode::InvalidParameter,
        constraint = user_token_account.owner == withdrawer.key() @ ErrorCode::Unauthorized,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    /// Fund's USDC vault
    #[account(
        mut,
        seeds = [FUND_VAULT_SEED],
        bump,
        constraint = fund_vault.key() == fund.fund_vault @ ErrorCode::InvalidParameter,
    )]
    pub fund_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<WithdrawFromFund>, shares: u64) -> Result<()> {
    require!(shares > 0, ErrorCode::InvalidAmount);

    let fund = &ctx.accounts.fund;

    require!(
        ctx.accounts.user_share_account.amount >= shares,
        ErrorCode::InsufficientShares
    );

    // Calculate USDC to return: usdc = shares * nav_per_share / WAD
    let usdc_amount = (shares as u128)
        .checked_mul(fund.nav_per_share)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(WAD)
        .ok_or(ErrorCode::DivisionByZero)?;

    let usdc_amount = u64::try_from(usdc_amount).map_err(|_| ErrorCode::MathOverflow)?;
    require!(usdc_amount > 0, ErrorCode::InvalidAmount);

    // Check vault has enough liquidity
    require!(
        ctx.accounts.fund_vault.amount >= usdc_amount,
        ErrorCode::InsufficientFundLiquidity
    );

    // Burn shares from withdrawer
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.share_mint.to_account_info(),
                from: ctx.accounts.user_share_account.to_account_info(),
                authority: ctx.accounts.withdrawer.to_account_info(),
            },
        ),
        shares,
    )?;

    // Transfer USDC from fund vault to withdrawer (Fund PDA signs)
    let fund_seeds = &[FUND_SEED, &[ctx.accounts.fund.bump]];
    let signer_seeds = &[&fund_seeds[..]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.fund_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.fund.to_account_info(),
            },
            signer_seeds,
        ),
        usdc_amount,
    )?;

    // Update fund state
    let fund = &mut ctx.accounts.fund;
    fund.total_shares = fund
        .total_shares
        .checked_sub(shares)
        .ok_or(ErrorCode::MathUnderflow)?;

    let clock = Clock::get()?;
    let fund_key = fund.key();
    let nav = fund.nav_per_share;
    emit!(FundWithdrawal {
        fund: fund_key,
        withdrawer: ctx.accounts.withdrawer.key(),
        shares_burned: shares,
        amount_returned: usdc_amount,
        nav_per_share: nav,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
