use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount, Transfer};
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::FundDeposit;
use crate::state::{Fund, FundStatus};

#[derive(Accounts)]
pub struct DepositToFund<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        seeds = [FUND_SEED],
        bump = fund.bump,
        constraint = fund.status == FundStatus::Active @ ErrorCode::FundPaused,
    )]
    pub fund: Account<'info, Fund>,

    /// Depositor's USDC token account
    #[account(
        mut,
        constraint = user_token_account.mint == fund.quote_mint @ ErrorCode::InvalidParameter,
        constraint = user_token_account.owner == depositor.key() @ ErrorCode::Unauthorized,
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

    /// Share token mint
    #[account(
        mut,
        seeds = [SHARE_MINT_SEED],
        bump,
        constraint = share_mint.key() == fund.share_mint @ ErrorCode::InvalidParameter,
    )]
    pub share_mint: Account<'info, Mint>,

    /// Depositor's share token account
    #[account(
        mut,
        constraint = user_share_account.mint == fund.share_mint @ ErrorCode::InvalidParameter,
        constraint = user_share_account.owner == depositor.key() @ ErrorCode::Unauthorized,
    )]
    pub user_share_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<DepositToFund>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let fund = &ctx.accounts.fund;

    // Calculate shares to mint
    // If no shares exist, 1:1 ratio. Otherwise: shares = amount * WAD / nav_per_share
    let shares_to_mint = if fund.total_shares == 0 {
        amount
    } else {
        let amount_wad = (amount as u128)
            .checked_mul(WAD)
            .ok_or(ErrorCode::MathOverflow)?;
        let shares_wad = amount_wad
            .checked_div(fund.nav_per_share)
            .ok_or(ErrorCode::DivisionByZero)?;
        u64::try_from(shares_wad).map_err(|_| ErrorCode::MathOverflow)?
    };

    require!(shares_to_mint > 0, ErrorCode::InvalidAmount);

    // Transfer USDC from depositor to fund vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.fund_vault.to_account_info(),
                authority: ctx.accounts.depositor.to_account_info(),
            },
        ),
        amount,
    )?;

    // Mint shares to depositor (Fund PDA signs as mint authority)
    let fund_seeds = &[FUND_SEED, &[ctx.accounts.fund.bump]];
    let signer_seeds = &[&fund_seeds[..]];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.share_mint.to_account_info(),
                to: ctx.accounts.user_share_account.to_account_info(),
                authority: ctx.accounts.fund.to_account_info(),
            },
            signer_seeds,
        ),
        shares_to_mint,
    )?;

    // Update fund state
    let fund = &mut ctx.accounts.fund;
    fund.total_deposits = fund
        .total_deposits
        .checked_add(amount)
        .ok_or(ErrorCode::MathOverflow)?;
    fund.total_shares = fund
        .total_shares
        .checked_add(shares_to_mint)
        .ok_or(ErrorCode::MathOverflow)?;

    let clock = Clock::get()?;
    let fund_key = fund.key();
    let nav = fund.nav_per_share;
    emit!(FundDeposit {
        fund: fund_key,
        depositor: ctx.accounts.depositor.key(),
        amount,
        shares_minted: shares_to_mint,
        nav_per_share: nav,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
