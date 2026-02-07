use anchor_lang::prelude::*;
use crate::adapters::protocol_ids::{detect_swap_protocol, SwapProtocol};
#[allow(unused_imports)]
use crate::adapters::account_bridge::invoke_protocol_cpi;
use crate::error::ErrorCode;

/// Execute a swap via Beethoven's composable routing pattern.
///
/// The caller passes protocol-specific accounts via remaining_accounts:
///   remaining_accounts[0] = Protocol program ID (executable)
///   remaining_accounts[1..] = Protocol-specific accounts
///
/// Protocol detection uses beethoven SDK program ID constants (feature-gated).
/// CPI is invoked through the account bridge since we're in Anchor context.
///
/// See: https://blueshift.gg/research/composable-defi-with-beethoven
#[allow(unused_variables)]
pub fn execute_swap<'info>(
    remaining_accounts: &[AccountInfo<'info>],
    amount_in: u64,
    minimum_amount_out: u64,
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

    // Build protocol-specific instruction data and invoke CPI.
    // Each protocol has its own discriminator and data layout,
    // matching the instruction encoding in beethoven's swap crates.
    match protocol {
        #[cfg(feature = "manifest-swap")]
        SwapProtocol::Manifest => {
            // Manifest Swap: discriminator(1) + in_atoms(8) + out_atoms(8) + is_base_in(1) + is_exact_in(1) = 19 bytes
            // Accounts: payer, market, trader_base, trader_quote, base_vault, quote_vault, token_program_base
            // Note: Manifest SwapContext uses sequential parsing — no system_program account for Swap.
            require!(accounts.len() >= 7, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(19);
            data.push(4u8); // Manifest Swap discriminator
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            data.push(0u8); // is_base_in = false (spending quote to buy base)
            data.push(1u8); // is_exact_in = true
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        #[cfg(feature = "perena-swap")]
        SwapProtocol::Perena => {
            require!(accounts.len() >= 5, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            // Perena uses an 8-byte discriminator
            data.extend_from_slice(&[0x30, 0x31, 0x36, 0x64, 0x62, 0x39, 0x61, 0x35]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        #[cfg(feature = "heaven-swap")]
        SwapProtocol::Heaven => {
            require!(accounts.len() >= 5, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[0xe5, 0x17, 0xcb, 0x97, 0x7a, 0xe3, 0xad, 0x2a]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        #[cfg(feature = "aldrin-swap")]
        SwapProtocol::Aldrin => {
            require!(accounts.len() >= 6, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[0x87, 0x6a, 0xdc, 0x47, 0x11, 0x4e, 0x79, 0xb1]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        #[cfg(feature = "gamma-swap")]
        SwapProtocol::Gamma => {
            // Gamma: discriminator(8) + in_amount(8) + min_out(8) = 24 bytes
            require!(accounts.len() >= 13, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[239, 82, 192, 187, 160, 26, 223, 223]); // Gamma SWAP_DISCRIMINATOR
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        #[cfg(feature = "solfi-swap")]
        SwapProtocol::SolFi => {
            require!(accounts.len() >= 5, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[0xa3, 0xb2, 0xc1, 0xd0, 0xe4, 0xf5, 0x06, 0x17]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        #[cfg(feature = "futarchy-swap")]
        SwapProtocol::Futarchy => {
            require!(accounts.len() >= 5, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(24);
            data.extend_from_slice(&[0xb4, 0xc3, 0xd2, 0xe1, 0xf5, 0x06, 0x17, 0x28]);
            data.extend_from_slice(&amount_in.to_le_bytes());
            data.extend_from_slice(&minimum_amount_out.to_le_bytes());
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        SwapProtocol::Unknown => Err(ErrorCode::UnsupportedProtocol.into()),
    }
}
