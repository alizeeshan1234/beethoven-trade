use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::SwapExecuted;
use crate::math::fixed_point::bps_mul;
use crate::adapters::swap_adapter;
use crate::state::{Exchange, UserAccount, VaultState};

use anchor_spl::token::{TokenAccount, Token};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ExecuteSwapParams {
    pub amount_in: u64,
    pub minimum_amount_out: u64,
}

#[derive(Accounts)]
pub struct ExecuteSwap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
        constraint = !exchange.swap_paused @ ErrorCode::ExchangePaused,
    )]
    pub exchange: Account<'info, Exchange>,

    #[account(
        mut,
        seeds = [USER_ACCOUNT_SEED, user.key().as_ref()],
        bump = user_account.bump,
        constraint = user_account.owner == user.key() @ ErrorCode::Unauthorized,
    )]
    pub user_account: Account<'info, UserAccount>,

    /// User's input token account
    #[account(mut)]
    pub user_input_token_account: Account<'info, TokenAccount>,

    /// User's output token account
    #[account(mut)]
    pub user_output_token_account: Account<'info, TokenAccount>,

    /// Vault for fee collection on input token
    #[account(
        mut,
        seeds = [VAULT_SEED, vault_state.mint.as_ref()],
        bump = vault_state.bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        constraint = vault_token_account.key() == vault_state.token_account @ ErrorCode::InvalidParameter,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, ExecuteSwap<'info>>,
    params: ExecuteSwapParams,
) -> Result<()> {
    require!(params.amount_in > 0, ErrorCode::InvalidAmount);

    let exchange = &ctx.accounts.exchange;
    let clock = Clock::get()?;

    // Calculate fee
    let fee = bps_mul(params.amount_in, exchange.swap_fee_bps)?;
    let amount_after_fee = params.amount_in
        .checked_sub(fee)
        .ok_or(ErrorCode::MathUnderflow)?;

    // Collect fee: transfer fee to vault
    if fee > 0 {
        anchor_spl::token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::Transfer {
                    from: ctx.accounts.user_input_token_account.to_account_info(),
                    to: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            fee,
        )?;

        // Track fees
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.collected_fees = vault_state
            .collected_fees
            .checked_add(fee)
            .ok_or(ErrorCode::MathOverflow)?;
    }

    // Record pre-swap output balance for slippage check
    let pre_balance = ctx.accounts.user_output_token_account.amount;

    // Execute swap via protocol adapter (remaining_accounts carries protocol program + accounts)
    swap_adapter::execute_swap(
        ctx.remaining_accounts,
        amount_after_fee,
        params.minimum_amount_out,
    )?;

    // Reload output token account to check post-swap balance
    ctx.accounts.user_output_token_account.reload()?;
    let post_balance = ctx.accounts.user_output_token_account.amount;

    let amount_out = post_balance
        .checked_sub(pre_balance)
        .ok_or(ErrorCode::MathUnderflow)?;

    require!(amount_out > 0, ErrorCode::SwapOutputZero);
    require!(
        amount_out >= params.minimum_amount_out,
        ErrorCode::SlippageExceeded
    );

    // Update user stats
    let user = &mut ctx.accounts.user_account;
    user.total_trades = user
        .total_trades
        .checked_add(1)
        .ok_or(ErrorCode::MathOverflow)?;
    user.total_volume = user
        .total_volume
        .checked_add(params.amount_in as u64)
        .ok_or(ErrorCode::MathOverflow)?;
    user.total_fees_paid = user
        .total_fees_paid
        .checked_add(fee)
        .ok_or(ErrorCode::MathOverflow)?;
    user.last_activity = clock.unix_timestamp;

    // Get protocol from remaining accounts for event
    let protocol = if !ctx.remaining_accounts.is_empty() {
        *ctx.remaining_accounts[0].key
    } else {
        Pubkey::default()
    };

    emit!(SwapExecuted {
        user: ctx.accounts.user.key(),
        input_mint: ctx.accounts.user_input_token_account.mint,
        output_mint: ctx.accounts.user_output_token_account.mint,
        amount_in: params.amount_in,
        amount_out,
        fee,
        protocol,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
