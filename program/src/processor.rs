use crate::error::Error;
use std::convert::TryInto;

use solana_program::msg;
use solana_program::program::invoke_signed;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    hash::hash,
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction::create_account,
};

use borsh::{BorshDeserialize, BorshSerialize};

use crate::instruction::{InitInstruction, MultiSigInstruction, ProposeInstruction};
use crate::state::{AccountType, GroupData, ProposalConfig, ProposalData, ProposalState};
use crate::utils::{read_account_data, write_account_data, write_serialized_data};

pub(crate) mod pda_tag {
    pub const GROUP: &[u8] = &[0];
    pub const PROPOSAL: &[u8] = &[1];
    pub const PROTECTED: &[u8] = &[2];
}

pub struct Processor<'a, 'b> {
    pub program_id: &'a Pubkey,
    pub accounts: &'a [AccountInfo<'b>],
}

impl Processor<'_, '_> {
    pub fn process(self, instruction_data: &[u8]) -> Result<(), Error> {
        let instruction = MultiSigInstruction::try_from_slice(instruction_data)
            .map_err(Error::InvalidInstructionData)?;

        match instruction {
            MultiSigInstruction::Init(instruction) => self.initialize(instruction),
            MultiSigInstruction::Propose(data) => self.propose(data),
            MultiSigInstruction::Approve(_) => self.approve(),
            MultiSigInstruction::CloseProposal(_) => self.close_proposal(),
        }
    }

    fn initialize(self, instruction: InitInstruction) -> Result<(), Error> {
        let Self {
            accounts,
            program_id,
        } = self;
        let InitInstruction {
            group_data: data,
            lamports,
            protected_account_config: init_protected,
        } = instruction;
        let accounts_iter = &mut accounts.iter();

        let initializer = next_account_info(accounts_iter)?;
        let group_account_info = next_account_info(accounts_iter)?;
        let _sys_program_account = next_account_info(accounts_iter)?;

        if !initializer.is_signer {
            return Err(ProgramError::MissingRequiredSignature.into());
        }

        if data.threshold == 0 {
            return Err(Error::ZeroThreshold);
        }
        // To make sure proposal bit mask is long enough.
        if data.members.len() > 64 {
            return Err(Error::TooManyMembers);
        }
        if data.members.is_empty() {
            return Err(Error::NoMembers);
        }
        for member in &data.members {
            if member.weight == 0 {
                return Err(Error::ZeroWeight);
            }
        }
        let weight_sum: u32 = data
            .members
            .iter()
            .map(|m| m.weight)
            .fold(0, |sum, weight| sum.saturating_add(weight));
        if weight_sum < data.threshold {
            return Err(Error::UnreachableThreshold);
        }

        let serialized_data = data.try_to_vec().map_err(Error::Serialize)?;
        let group_seed = hash(&serialized_data);

        let (addr, nonce) =
            Pubkey::find_program_address(&[pda_tag::GROUP, group_seed.as_ref()], program_id);

        if *group_account_info.key != addr {
            return Err(Error::InvalidGroupAccountKey);
        }

        // Create account to hold group data
        let create_instruction = create_account(
            initializer.key,
            &addr,
            lamports,
            serialized_data.len() as u64 + 1,
            program_id,
        );

        invoke_signed(
            &create_instruction,
            accounts,
            &[&[pda_tag::GROUP, group_seed.as_ref(), &[nonce]]],
        )?;

        write_serialized_data(group_account_info, AccountType::Group, &serialized_data)?;

        if let Some(protected_account_config) = init_protected {
            let protected_account = next_account_info(accounts_iter)?;
            let (protected_key, nonce) =
                Pubkey::find_program_address(&[pda_tag::PROTECTED, addr.as_ref()], program_id);

            if *protected_account.key != protected_key {
                return Err(Error::InvalidProtectedAccountKey);
            }

            let create_instruction = create_account(
                initializer.key,
                &protected_key,
                protected_account_config.lamports,
                protected_account_config.space,
                &protected_account_config.owner,
            );

            invoke_signed(
                &create_instruction,
                accounts,
                &[&[pda_tag::PROTECTED, addr.as_ref(), &[nonce]]],
            )?;
        }

        Ok(())
    }

    fn propose(self, data: ProposeInstruction) -> Result<(), Error> {
        let Self {
            accounts,
            program_id,
        } = self;
        let accounts_iter = &mut accounts.iter();

        let signer_account_info = next_account_info(accounts_iter)?;
        if !signer_account_info.is_signer {
            return Err(ProgramError::MissingRequiredSignature.into());
        }

        let group_account_info = next_account_info(accounts_iter)?;
        let group_data = check_and_read_group_data(group_account_info, program_id)?;

        let (signer_index, signer_weight) = group_data.weight(signer_account_info.key)?;

        let mut state = ProposalState::new();
        state.add_approval(signer_index, signer_weight)?;
        if state.current_weight() >= group_data.threshold {
            let (_protected_pubkey, seed) = Pubkey::find_program_address(
                &[pda_tag::PROTECTED, group_account_info.key.as_ref()],
                program_id,
            );

            for instruction in data.instructions {
                invoke_signed(
                    &instruction.try_into()?,
                    accounts,
                    &[&[pda_tag::PROTECTED, group_account_info.key.as_ref(), &[seed]]],
                )?;
            }
        } else {
            let proposal_account_info = next_account_info(accounts_iter)?;

            let config = ProposalConfig {
                group: *group_account_info.key,
                instructions: data.instructions,
                author: *signer_account_info.key,
                salt: data.salt
            };
            // Note: This is proposed instruction, not proposal data
            let serialized_config = config.try_to_vec().map_err(Error::Serialize)?;
            let proposal_hash = hash(&serialized_config);

            let proposal = ProposalData { config, state };

            let (addr, nonce) = Pubkey::find_program_address(
                &[pda_tag::PROPOSAL, proposal_hash.as_ref()],
                program_id,
            );

            if *proposal_account_info.key != addr {
                return Err(Error::InvalidProposalAccountKey);
            }

            let serialized_data = proposal.try_to_vec().map_err(Error::Serialize)?;

            let create_instruction = create_account(
                signer_account_info.key,
                &addr,
                data.lamports,
                serialized_data.len() as u64 + 1,
                program_id,
            );

            invoke_signed(
                &create_instruction,
                accounts,
                &[&[pda_tag::PROPOSAL, proposal_hash.as_ref(), &[nonce]]],
            )?;

            write_serialized_data(
                proposal_account_info,
                AccountType::Proposal,
                &serialized_data,
            )?;
        }

        Ok(())
    }

    fn approve(self) -> Result<(), Error> {
        let Self {
            accounts,
            program_id,
        } = self;
        let accounts_iter = &mut accounts.iter();

        let signer_account_info = next_account_info(accounts_iter)?;
        if !signer_account_info.is_signer {
            return Err(ProgramError::MissingRequiredSignature.into());
        }

        let group_account_info = next_account_info(accounts_iter)?;
        let group_data = check_and_read_group_data(group_account_info, program_id)?;
        let (signer_index, signer_weight) = group_data.weight(signer_account_info.key)?;

        let proposal_account_info = next_account_info(accounts_iter)?;
        let proposal_data = check_and_read_proposal_data(proposal_account_info, program_id)?;
        let ProposalData {
            config: proposal_config,
            state: mut proposal_state,
        } = proposal_data;

        let protected_account_info = next_account_info(accounts_iter)?;

        if proposal_config.group != *group_account_info.key {
            return Err(Error::InvalidGroupAccountKey);
        }

        proposal_state.add_approval(signer_index, signer_weight)?;
        if proposal_state.current_weight() >= group_data.threshold {
            let (_protected_pubkey, seed) = Pubkey::find_program_address(
                &[pda_tag::PROTECTED, group_account_info.key.as_ref()],
                program_id,
            );

            for instruction in proposal_config.instructions {
                invoke_signed(
                    &instruction.try_into()?,
                    accounts,
                    &[&[pda_tag::PROTECTED, group_account_info.key.as_ref(), &[seed]]],
                )?;
            }

            for i in &mut **proposal_account_info.data.borrow_mut() {
                *i = 0;
            }

            transfer_lamports_from_proposal(proposal_account_info, protected_account_info);
        } else {
            let proposal_data = ProposalData {
                config: proposal_config,
                state: proposal_state,
            };
            write_account_data(proposal_account_info, AccountType::Proposal, &proposal_data)?;
        }

        Ok(())
    }

    fn close_proposal(self) -> Result<(), Error> {
        let Self {
            accounts,
            program_id,
        } = self;
        let accounts_iter = &mut accounts.iter();

        let signer_account_info = next_account_info(accounts_iter)?;
        if !signer_account_info.is_signer {
            return Err(ProgramError::MissingRequiredSignature.into());
        }

        let proposal_account_info = next_account_info(accounts_iter)?;
        let proposal_data = check_and_read_proposal_data(proposal_account_info, program_id)?;
        let proposal_config = proposal_data.config;
        let author = proposal_config.author;
        if signer_account_info.key != &author {
            return Err(ProgramError::MissingRequiredSignature.into());
        }

        let destination_account_info = next_account_info(accounts_iter)?;

        for i in &mut **proposal_account_info.data.borrow_mut() {
            *i = 0;
        }

        transfer_lamports_from_proposal(proposal_account_info, destination_account_info);
        Ok(())
    }
}

fn check_and_read_group_data(info: &AccountInfo, program_id: &Pubkey) -> Result<GroupData, Error> {
    if info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId.into());
    }

    let group_data = read_account_data::<GroupData>(AccountType::Group, info)?;

    let group_hash = hash(&info.data.borrow()[1..]);

    let (group_pda, _) =
        Pubkey::find_program_address(&[pda_tag::GROUP, group_hash.as_ref()], program_id);
    if group_pda != *info.key {
        return Err(Error::InvalidGroupAccountKey);
    }
    Ok(group_data)
}

pub fn check_and_read_proposal_data(
    info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<ProposalData, Error> {
    if info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId.into());
    }

    let proposal_data = read_account_data::<ProposalData>(AccountType::Proposal, info)?;

    // TODO: Partial deserialize?
    let serialized_instruction = proposal_data
        .config
        .try_to_vec()
        .map_err(Error::Serialize)?;
    let proposal_hash = hash(&serialized_instruction);

    let (addr, _) =
        Pubkey::find_program_address(&[pda_tag::PROPOSAL, proposal_hash.as_ref()], program_id);
    if addr != *info.key {
        return Err(Error::InvalidProposalAccountKey);
    }

    Ok(proposal_data)
}

/// Transfer lamports back to a destination account.
/// This happens when a proposal is closed.
fn transfer_lamports_from_proposal(
    proposal_account_info: &AccountInfo,
    destination_account_info: &AccountInfo,
) {
    let lamports = **proposal_account_info.lamports.borrow();
    msg!(
        "transferring {} lamports to close the proposal account",
        lamports
    );
    **proposal_account_info.lamports.borrow_mut() = 0;
    **destination_account_info.lamports.borrow_mut() += lamports;
}
