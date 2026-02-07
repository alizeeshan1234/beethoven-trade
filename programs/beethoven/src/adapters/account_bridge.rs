use anchor_lang::prelude::*;

/// Bridge between Anchor's AccountInfo and Beethoven's CPI patterns.
///
/// Beethoven SDK uses pinocchio's `AccountView` (from `solana-account-view`)
/// which reads directly from the raw BPF entrypoint input buffer.
/// Anchor uses `solana-program`'s `AccountInfo` which has its own struct layout.
///
/// Since both types wrap the same underlying runtime account data, this bridge
/// constructs CPI calls using `solana_program::program::invoke()` — the same
/// operation beethoven performs internally — while preserving protocol detection
/// from beethoven's feature-gated program ID constants.
///
/// The remaining_accounts pattern:
///   remaining_accounts[0] = Protocol program (executable) — beethoven checks this
///   remaining_accounts[1..] = Protocol-specific accounts (vaults, mints, etc.)

/// Build a CPI instruction from remaining_accounts and invoke it.
/// This is the Anchor-compatible equivalent of beethoven's `invoke_signed()`.
pub fn invoke_protocol_cpi<'info>(
    program: &AccountInfo<'info>,
    accounts: &[AccountInfo<'info>],
    data: Vec<u8>,
) -> Result<()> {
    let account_metas: Vec<AccountMeta> = accounts
        .iter()
        .map(|a| {
            if a.is_writable {
                AccountMeta::new(*a.key, a.is_signer)
            } else {
                AccountMeta::new_readonly(*a.key, a.is_signer)
            }
        })
        .collect();

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: *program.key,
        accounts: account_metas,
        data,
    };

    let mut all_accounts = vec![program.clone()];
    all_accounts.extend_from_slice(accounts);

    anchor_lang::solana_program::program::invoke(&ix, &all_accounts)?;
    Ok(())
}

/// Build a CPI instruction from remaining_accounts and invoke with PDA signer seeds.
pub fn invoke_protocol_cpi_signed<'info>(
    program: &AccountInfo<'info>,
    accounts: &[AccountInfo<'info>],
    data: Vec<u8>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let account_metas: Vec<AccountMeta> = accounts
        .iter()
        .map(|a| {
            if a.is_writable {
                AccountMeta::new(*a.key, a.is_signer)
            } else {
                AccountMeta::new_readonly(*a.key, a.is_signer)
            }
        })
        .collect();

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: *program.key,
        accounts: account_metas,
        data,
    };

    let mut all_accounts = vec![program.clone()];
    all_accounts.extend_from_slice(accounts);

    anchor_lang::solana_program::program::invoke_signed(&ix, &all_accounts, signer_seeds)?;
    Ok(())
}
