use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey,
};

use processor::Processor;

mod error;
pub mod instruction;
mod processor;
pub mod state;
mod utils;

#[cfg(test)]
mod tests;

// Declare and export the program's entrypoint
entrypoint!(process_instruction);

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("hello from multisig v3");
    let processor = Processor {
        accounts,
        program_id,
    };
    processor.process(instruction_data).map_err(|err| {
        msg!("multisig program error: {}", err);
        ProgramError::from(err)
    })
}
