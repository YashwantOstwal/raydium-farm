use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    OpenTimeHasToBeInFuture,

    #[msg("Custom error message")]
    CloseTimeHasToBeGreaterThanOpenTime,  

    #[msg("Custom error message")]
    InsufficientBalance,

    #[msg("Custom error message")]
    MismatchingAccounts,

    #[msg("Custom error message")]
    InvalidAmount
}
