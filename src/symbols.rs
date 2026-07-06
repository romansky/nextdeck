pub const ENABLED: char = '✓';
pub const DISABLED: char = '✗';

pub const fn bool_symbol(value: bool) -> char {
    if value { ENABLED } else { DISABLED }
}
