use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Decibels(f32);

impl Decibels {
    #[must_use]
    pub fn from_amplitude(amp: f32) -> Self {
        Self(if amp < f32::EPSILON {
            f32::NEG_INFINITY
        } else {
            20.0 * amp.log10()
        })
    }

    #[must_use]
    pub fn to_amplitude(self) -> f32 {
        if self.0 == f32::NEG_INFINITY {
            0.0
        } else {
            10f32.powf(0.05 * self.0)
        }
    }
}

impl Display for Decibels {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2} dB", self.0)
    }
}
