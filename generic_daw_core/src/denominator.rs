use atomig::Atom;
use std::fmt::{Display, Formatter};
use strum::VariantArray;

#[repr(u8)]
#[derive(Atom, Clone, Copy, Debug, Default, Eq, PartialEq, VariantArray)]
pub enum Denominator {
    _2 = 2,
    #[default]
    _4 = 4,
    _8 = 8,
    _16 = 16,
}

impl Display for Denominator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u16)
    }
}
