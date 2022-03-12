use crate::error::Error;
use std::convert::TryFrom;

use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use borsh::{BorshDeserialize, BorshSerialize};

#[repr(u8)]
pub enum AccountType {
    Group = 1,
    Proposal = 2,
}

impl From<AccountType> for u8 {
    #[inline]
    fn from(value: AccountType) -> Self {
        value as u8
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct GroupData {
    pub members: Vec<GroupMember>,
    pub threshold: u32,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct GroupMember {
    pub public_key: Pubkey,
    pub weight: u32,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposalState {
    members: u64,
    current_weight: u32,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposalConfig {
    pub group: Pubkey,
    pub instructions: Vec<ProposedInstruction>,
    pub author: Pubkey,
    pub salt: u64
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposalData {
    pub config: ProposalConfig,
    pub state: ProposalState,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct ProposedInstruction {
    pub program_id: Pubkey,
    pub accounts: Vec<ProposedAccountMeta>,
    pub data: Vec<u8>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct ProposedAccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl GroupData {
    pub fn weight(&self, key: &Pubkey) -> Result<(usize, u32), Error> {
        self.members
            .iter()
            .enumerate()
            .find(|(_i, member)| member.public_key == *key)
            .map(|(i, member)| (i, member.weight))
            .ok_or(Error::Unauthorized)
    }
}

impl ProposalState {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            current_weight: 0,
            members: 0,
        }
    }

    pub fn add_approval(&mut self, idx: usize, weight: u32) -> Result<(), Error> {
        if self.members & (1 << idx as u64) != 0 {
            return Err(Error::AlreadyParticipate);
        }
        self.members |= 1 << idx as u64;
        self.current_weight = self.current_weight.saturating_add(weight);
        Ok(())
    }

    #[inline]
    pub fn current_weight(&self) -> u32 {
        self.current_weight
    }

    #[cfg(test)]
    pub fn is_approved_by(&self, idx: usize) -> bool {
        self.members & (1 << idx as u64) != 0
    }
}

impl TryFrom<ProposedInstruction> for Instruction {
    type Error = Error;

    fn try_from(data: ProposedInstruction) -> Result<Self, Self::Error> {
        Ok(Instruction {
            program_id: data.program_id,
            accounts: data
                .accounts
                .iter()
                .map(|account| {
                    Ok(AccountMeta {
                        is_signer: account.is_signer,
                        is_writable: account.is_writable,
                        pubkey: account.pubkey,
                    })
                })
                .collect::<Result<_, Self::Error>>()?,
            data: data.data,
        })
    }
}
