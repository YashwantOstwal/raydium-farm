
pub fn to_x64(num:u64) -> u128 {
    (num as u128).checked_shl(64).unwrap()
}

pub fn from_x64(num:u128) -> u64 {
    num.checked_shr(64).unwrap() as u64
}

pub fn div_x64(numerator:u128) -> u64 {
    from_x64(numerator)
}

pub fn ceil_div_x64(numerator:u128) -> u64 {
    if numerator == 0 {
        0
    } else {
        (numerator.checked_sub(1).unwrap().checked_shr(64).unwrap()).checked_add(1).unwrap() as u64
    }
}

pub fn duration(end_time:i64,open_time:i64) -> u64 {
    end_time.checked_sub(open_time).unwrap().abs() as u64
}
