use crate::constants::X64;

pub fn to_x64(num:u64) -> u128 {
    num.checked_shl(64).unwrap() as u128
}

pub fn from_x64(num:u128) -> u64 {
    num.checked_shr(64).unwrap() as u64
}

pub fn div_x64(numerator:u128) -> u64 {
    from_x64(numerator)
}

pub fn ceil_div_x64(numerator:u128) -> u64 {
    // formula -> (numerator + 2^64 - 1) / 2^64
    numerator.checked_add(X64).unwrap().checked_sub(1u128).unwrap().checked_shr(64).unwrap() as u64
}

pub fn duration(end_time:i64,open_time:i64) -> u64 {
    end_time.checked_sub(open_time).unwrap().abs() as u64
}
