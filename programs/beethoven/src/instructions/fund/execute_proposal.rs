use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::ProposalExecuted;
use crate::state::{Exchange, Fund, Proposal, ProposalStatus, ActionType};
use crate::state::proposal::{SwapActionData, PerpActionData, LendingActionData};
use super::fund_actions;

#[derive(Accounts)]
pub struct ExecuteProposal<'info> {
    /// Permissionless executor
    pub executor: Signer<'info>,

    #[account(
        mut,
        seeds = [FUND_SEED],
        bump = fund.bump,
    )]
    pub fund: Box<Account<'info, Fund>>,

    #[account(
        mut,
        seeds = [
            FUND_PROPOSAL_SEED,
            fund.key().as_ref(),
            &proposal.proposal_index.to_le_bytes(),
        ],
        bump = proposal.bump,
        constraint = proposal.fund == fund.key() @ ErrorCode::InvalidParameter,
        constraint = proposal.status == ProposalStatus::Passed @ ErrorCode::ProposalNotPassed,
    )]
    pub proposal: Box<Account<'info, Proposal>>,

    #[account(
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
    )]
    pub exchange: Box<Account<'info, Exchange>>,

    /// Fund's USDC vault
    #[account(
        mut,
        seeds = [FUND_VAULT_SEED],
        bump,
        constraint = fund_vault.key() == fund.fund_vault @ ErrorCode::InvalidParameter,
    )]
    pub fund_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    // remaining_accounts: action-specific accounts
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, ExecuteProposal<'info>>,
) -> Result<()> {
    let clock = Clock::get()?;
    let proposal = &ctx.accounts.proposal;

    // Verify execution deadline hasn't passed
    require!(
        clock.unix_timestamp <= proposal.execution_deadline,
        ErrorCode::ProposalExpired
    );

    let fund_bump = ctx.accounts.fund.bump;
    let fund_seeds: &[&[u8]] = &[FUND_SEED, &[fund_bump]];
    let fund_key = ctx.accounts.fund.key();

    // Dispatch based on action type
    match proposal.action_type {
        ActionType::Swap => {
            let action_data = SwapActionData::try_from_slice(&proposal.action_data)
                .map_err(|_| ErrorCode::InvalidActionData)?;
            fund_actions::execute_fund_swap(
                &action_data,
                ctx.remaining_accounts,
                fund_seeds,
            )?;
        }
        ActionType::OpenPerp => {
            let action_data = PerpActionData::try_from_slice(&proposal.action_data)
                .map_err(|_| ErrorCode::InvalidActionData)?;
            fund_actions::execute_fund_open_perp(
                &action_data,
                ctx.remaining_accounts,
                &fund_key,
            )?;
        }
        ActionType::ClosePerp => {
            let action_data = PerpActionData::try_from_slice(&proposal.action_data)
                .map_err(|_| ErrorCode::InvalidActionData)?;
            fund_actions::execute_fund_close_perp(
                &action_data,
                ctx.remaining_accounts,
                &fund_key,
            )?;
        }
        ActionType::DepositLending => {
            let action_data = LendingActionData::try_from_slice(&proposal.action_data)
                .map_err(|_| ErrorCode::InvalidActionData)?;
            fund_actions::execute_fund_deposit_lending(
                &action_data,
                ctx.remaining_accounts,
                fund_seeds,
            )?;
        }
        ActionType::WithdrawLending => {
            let action_data = LendingActionData::try_from_slice(&proposal.action_data)
                .map_err(|_| ErrorCode::InvalidActionData)?;
            fund_actions::execute_fund_withdraw_lending(
                &action_data,
                ctx.remaining_accounts,
                fund_seeds,
            )?;
        }
        ActionType::Borrow => {
            let action_data = LendingActionData::try_from_slice(&proposal.action_data)
                .map_err(|_| ErrorCode::InvalidActionData)?;
            fund_actions::execute_fund_deposit_lending(
                &action_data,
                ctx.remaining_accounts,
                fund_seeds,
            )?;
        }
        ActionType::Repay => {
            let action_data = LendingActionData::try_from_slice(&proposal.action_data)
                .map_err(|_| ErrorCode::InvalidActionData)?;
            fund_actions::execute_fund_withdraw_lending(
                &action_data,
                ctx.remaining_accounts,
                fund_seeds,
            )?;
        }
        ActionType::UpdateParam => {
            // Parameter updates handled separately by admin
            msg!("UpdateParam action: not yet implemented");
        }
    }

    // Capture keys before mutable borrow
    let fund_key_for_event = ctx.accounts.fund.key();
    let proposal_key_for_event = ctx.accounts.proposal.key();
    let action_type_for_event = ctx.accounts.proposal.action_type as u8;

    // Mark proposal as executed
    let proposal = &mut ctx.accounts.proposal;
    proposal.status = ProposalStatus::Executed;
    proposal.executed_at = clock.unix_timestamp;

    emit!(ProposalExecuted {
        fund: fund_key_for_event,
        proposal: proposal_key_for_event,
        action_type: action_type_for_event,
        executor: ctx.accounts.executor.key(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
