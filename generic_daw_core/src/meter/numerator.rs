use atomig::Atom;
use std::fmt::{Display, Formatter};

#[repr(u8)]
#[derive(Atom, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Numerator {
    _1 = 1,
    _2 = 2,
    _3 = 3,
    #[default]
    _4 = 4,
    _5 = 5,
    _6 = 6,
    _7 = 7,
    _8 = 8,
    _9 = 9,
    _10 = 10,
    _11 = 11,
    _12 = 12,
    _13 = 13,
    _14 = 14,
    _15 = 15,
    _16 = 16,
}

impl Numerator {
    pub const VARIANTS: [Self; 16] = [
        Self::_1,
        Self::_2,
        Self::_3,
        Self::_4,
        Self::_5,
        Self::_6,
        Self::_7,
        Self::_8,
        Self::_9,
        Self::_10,
        Self::_11,
        Self::_12,
        Self::_13,
        Self::_14,
        Self::_15,
        Self::_16,
    ];
}

impl Display for Numerator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u8)
    }
}
