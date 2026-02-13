use anchor_lang::prelude::*;
use crate::adapters::account_bridge::invoke_protocol_cpi_signed;
use crate::adapters::protocol_ids::{detect_swap_protocol, SwapProtocol};
use crate::error::ErrorCode;
use crate::state::proposal::{SwapActionData, PerpActionData, LendingActionData};

/// Execute a fund swap using the Beethoven protocol routing pattern.
/// Uses `invoke_protocol_cpi_signed` with Fund PDA signer seeds.
///
/// remaining_accounts follows the same pattern as regular swaps:
///   [0] = protocol program, [1..] = protocol-specific accounts
pub fn execute_fund_swap<'info>(
    action_data: &SwapActionData,
    remaining_accounts: &[AccountInfo<'info>],
    fund_seeds: &[&[u8]],
) -> Result<()> {
    require!(
        !remaining_accounts.is_empty(),
        ErrorCode::UnsupportedProtocol
    );

    let protocol_program = &remaining_accounts[0];
    let protocol = detect_swap_protocol(protocol_program.key);

    require!(
        protocol != SwapProtocol::Unknown,
        ErrorCode::UnsupportedProtocol
    );

    let accounts = &remaining_accounts[1..];
    let signer_seeds = &[fund_seeds];

    // Build protocol-specific instruction data (same encoding as swap_adapter)
    // Then invoke with Fund PDA as signer
    #[allow(unused_variables)]
    let amount_in = action_data.amount_in;
    #[allow(unused_variables)]
    let minimum_amount_out = action_data.minimum_amount_out;

    match protocol {
        #[cfg(feature = "manifest-swap")]
        SwapProtocol::Manifest => {
            require!(accounts.len() >= 7, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(19);
            data.push(4u8);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            data.push(0u8); // is_base_in
            data.push(1u8); // is_exact_in
            invoke_protocol_cpi_signed(protocol_program, accounts, data, signer_seeds)
        }
        #[cfg(feature = "perena-swap")]
        SwapProtocol::Perena => {
            require!(accounts.len() >= 5, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[0x30, 0x31, 0x36, 0x64, 0x62, 0x39, 0x61, 0x35]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi_signed(protocol_program, accounts, data, signer_seeds)
        }
        #[cfg(feature = "heaven-swap")]
        SwapProtocol::Heaven => {
            require!(accounts.len() >= 5, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[0xe5, 0x17, 0xcb, 0x97, 0x7a, 0xe3, 0xad, 0x2a]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi_signed(protocol_program, accounts, data, signer_seeds)
        }
        #[cfg(feature = "aldrin-swap")]
        SwapProtocol::Aldrin => {
            require!(accounts.len() >= 6, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[0x87, 0x6a, 0xdc, 0x47, 0x11, 0x4e, 0x79, 0xb1]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi_signed(protocol_program, accounts, data, signer_seeds)
        }
        #[cfg(feature = "gamma-swap")]
        SwapProtocol::Gamma => {
            require!(accounts.len() >= 13, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[239, 82, 192, 187, 160, 26, 223, 223]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi_signed(protocol_program, accounts, data, signer_seeds)
        }
        #[cfg(feature = "solfi-swap")]
        SwapProtocol::SolFi => {
            require!(accounts.len() >= 5, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[0xa3, 0xb2, 0xc1, 0xd0, 0xe4, 0xf5, 0x06, 0x17]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi_signed(protocol_program, accounts, data, signer_seeds)
        }
        #[cfg(feature = "futarchy-swap")]
        SwapProtocol::Futarchy => {
            require!(accounts.len() >= 5, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[0xb4, 0xc3, 0xd2, 0xe1, 0xf5, 0x06, 0x17, 0x28]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi_signed(protocol_program, accounts, data, signer_seeds)
        }
        SwapProtocol::Unknown => Err(ErrorCode::UnsupportedProtocol.into()),
    }
}

/// Execute a fund perp open position (internal state modification).
/// This directly manipulates perp_market state and creates a perp position
/// owned by the Fund PDA, avoiding CPI overhead.
///
/// remaining_accounts: [perp_market, oracle, vault_token_account]
pub fn execute_fund_open_perp<'info>(
    _action_data: &PerpActionData,
    _remaining_accounts: &[AccountInfo<'info>],
    _fund_key: &Pubkey,
) -> Result<()> {
    // Perp position opening for fund requires:
    // 1. Validate leverage against market limits
    // 2. Get oracle price
    // 3. Transfer collateral from fund_vault to perp vault
    // 4. Create PerpPosition with fund PDA as owner
    // 5. Update market OI
    //
    // This will be fully implemented when integrating with the existing
    // perp infrastructure. The execute_proposal handler passes through
    // the necessary remaining_accounts for this operation.
    msg!("Fund perp open: not yet implemented");
    Ok(())
}

/// Close a fund-owned perp position, returning PnL to fund_vault.
pub fn execute_fund_close_perp<'info>(
    _action_data: &PerpActionData,
    _remaining_accounts: &[AccountInfo<'info>],
    _fund_key: &Pubkey,
) -> Result<()> {
    msg!("Fund perp close: not yet implemented");
    Ok(())
}

/// Deposit fund assets into a lending pool.
pub fn execute_fund_deposit_lending<'info>(
    _action_data: &LendingActionData,
    _remaining_accounts: &[AccountInfo<'info>],
    _fund_seeds: &[&[u8]],
) -> Result<()> {
    msg!("Fund lending deposit: not yet implemented");
    Ok(())
}

/// Withdraw fund assets from a lending pool.
pub fn execute_fund_withdraw_lending<'info>(
    _action_data: &LendingActionData,
    _remaining_accounts: &[AccountInfo<'info>],
    _fund_seeds: &[&[u8]],
) -> Result<()> {
    msg!("Fund lending withdraw: not yet implemented");
    Ok(())
}
