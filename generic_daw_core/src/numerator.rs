use atomig::Atom;
use std::fmt::Display;
use strum::VariantArray;

#[repr(u8)]
#[derive(Atom, Clone, Copy, Debug, Default, Eq, PartialEq, VariantArray)]
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

impl Display for Numerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", *self as u16)
    }
}
