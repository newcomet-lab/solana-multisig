use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid instruction data: {0}")]
    InvalidInstructionData(#[source] std::io::Error),
    #[error("invalid account data: {0}")]
    AccountDataDeserialize(#[source] std::io::Error),
    #[error("serialization failed: {0}")]
    Serialize(#[source] std::io::Error),
    #[error("{0}")]
    Program(#[from] ProgramError),
    #[error("threshold can't be zero")]
    ZeroThreshold,
    #[error("too many group members")]
    TooManyMembers,
    #[error("no group members")]
    NoMembers,
    #[error("weight can't be zero")]
    ZeroWeight,
    #[error("threshold is unreachable")]
    UnreachableThreshold,
    #[error("invalid group account key")]
    InvalidGroupAccountKey,
    #[error("invalid proposal account key")]
    InvalidProposalAccountKey,
    #[error("invalid protected account key")]
    InvalidProtectedAccountKey,
    #[error("unauthorized")]
    Unauthorized,
    #[error("you already participate in this proposal")]
    AlreadyParticipate,
    #[error("invalid account type")]
    InvalidAccountType,
    #[error("empty account data")]
    EmptyAccountData,
}

impl From<Error> for ProgramError {
    fn from(error: Error) -> Self {
        match error {
            Error::Program(error) => error,
            Error::InvalidInstructionData(_) => ProgramError::InvalidInstructionData,
            Error::AccountDataDeserialize(_)
            | Error::InvalidAccountType
            | Error::EmptyAccountData => ProgramError::InvalidAccountData,
            Error::Serialize(_) => ProgramError::Custom(1),
            Error::ZeroThreshold
            | Error::TooManyMembers
            | Error::NoMembers
            | Error::ZeroWeight
            | Error::UnreachableThreshold
            | Error::InvalidGroupAccountKey
            | Error::InvalidProposalAccountKey
            | Error::InvalidProtectedAccountKey
            | Error::Unauthorized
            | Error::AlreadyParticipate => ProgramError::InvalidArgument,
        }
    }
}
