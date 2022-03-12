use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

use crate::state::{GroupData, ProposedInstruction};

#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub struct ProtectedAccountConfig {
    /// Amount of lamports to fund the protected account.
    pub lamports: u64,
    /// Amount of space to allocate for protected account.
    pub space: u64,
    /// Id of the program to be set as an owner of protected account.
    pub owner: Pubkey,
}

/// Create and initialize group account.
///
/// # Account references
///   0. `[WRITE, SIGNER]` Initializer account. Will be used as a funding account for group account creation.
///   1. `[WRITE]` Group account to initialize. Key must be a PDA seeded by group data hash. Must not exist.
///   2. `[]` System program account.
///   3. `[WRITE]` (Optional) Protected account if applicable.
#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub struct InitInstruction {
    /// Group configuration data.
    pub group_data: GroupData,
    /// Amount of lamports to fund the new group account.
    pub lamports: u64,
    /// Optional config to create protected account with.
    pub protected_account_config: Option<ProtectedAccountConfig>,
}

/// Propose instruction to be executed by the group.
///
/// # Account references
///   0. `[SIGNER, WRITE]` Proposer account. Must be a member of the group.
///   1. `[WRITE]` Group account. TODO: Remove writable
///   2. `[WRITE]` Proposal account. Must not exist. Key must be a PDA seeded by proposed instruction
///                and invoked program_id (See [ProposalConfig](crate::state::ProposalConfig)).
///   3. `[]` System program account.
///   4. `[]` Proposed instruction program account.
///   5. ..5+N `[]` N accounts needed for proposed instructions to succeed.
#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub struct ProposeInstruction {
    /// Instruction to be proposed.
    pub instructions: Vec<ProposedInstruction>,
    /// Amount of lamports to fund the new proposal account.
    pub lamports: u64,
    /// A salt that will make this proposal unique.
    pub salt: u64
}

/// Approve already proposed instruction.
///
/// # Account references
///   0. `[SIGNER, WRITE]` Approver account. Must be a member of the group.
///   1. `[WRITE]` Group account. TODO: Remove writable
///   2. `[WRITE]` Proposal account that holds instruction to be approved.
///   3. `[WRITE]` Protected group account. Used to transfer lamports back to if the proposal is closed.
///   4. `[]` Proposed instruction program account.
///   5. ..5+N `[]` N accounts needed for proposed instruction to succeed.
#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub struct ApproveInstruction {} // TODO?: is this unit?

/// Closes a proposal and transfers its lamports to a group's protected address.
/// # Account references
///   0. `[SIGNER, WRITE]` Closer account. Must be a member of the group. Must be the same one who created the proposal.
///   2. `[WRITE]` Proposal account that holds instruction to be approved.
///   3. `[WRITE]` Destination account. Will receive lamports that proposal account has.
#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub struct CloseProposalInstruction {}

#[derive(BorshDeserialize, BorshSerialize)]
pub enum MultiSigInstruction {
    Init(InitInstruction),
    Propose(ProposeInstruction),
    Approve(ApproveInstruction),
    CloseProposal(CloseProposalInstruction),
}
