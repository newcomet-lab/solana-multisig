use crate::error::Error;

use solana_program::{account_info::AccountInfo, program_error::ProgramError};

use crate::state::AccountType;
use borsh::{BorshDeserialize, BorshSerialize};

pub fn read_account_data<T: BorshDeserialize>(
    account_type: AccountType,
    info: &AccountInfo,
) -> Result<T, Error> {
    let buf = info.data.borrow();
    if buf.is_empty() {
        return Err(Error::EmptyAccountData);
    }
    if buf[0] != u8::from(account_type) {
        return Err(Error::InvalidAccountType);
    }
    T::try_from_slice(&buf[1..]).map_err(Error::AccountDataDeserialize)
}

pub fn write_serialized_data(
    info: &AccountInfo,
    account_type: AccountType,
    data: &[u8],
) -> Result<(), Error> {
    let mut buf = info.data.borrow_mut();
    if buf.len() < data.len() + 1 {
        return Err(ProgramError::AccountDataTooSmall.into());
    }
    buf[0] = account_type.into();
    buf[1..data.len() + 1].copy_from_slice(data);
    for x in &mut buf[data.len() + 1..] {
        *x = 0; // TODO: use `fill()` (need Rust 1.50)
    }
    Ok(())
}

pub fn write_account_data<T: BorshSerialize>(
    info: &AccountInfo,
    account_type: AccountType,
    data: &T,
) -> Result<(), Error> {
    let data = data.try_to_vec().map_err(Error::Serialize)?;
    write_serialized_data(info, account_type, &data)
}
