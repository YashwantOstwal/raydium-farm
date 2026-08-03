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
    InvalidAmount,

    #[msg("Custom error message")]
    RewardStreamsLimitExceeded,

    #[msg("Custom error message")]
    ReferencedRewardStreamInvalid,

    #[msg("Custom error message")]
    RewardStreamAlreadyEnded,

    #[msg("Custom error message")]
    OpenTimeCannotBeModified,

    #[msg("Custom error message")]
    CannotShrinkEndTime,

    #[msg("Custom error message")]
    CannotLowerEmissionPerSecond,

    #[msg("Custom error message")]
    RewardStreamIsRunning
}
