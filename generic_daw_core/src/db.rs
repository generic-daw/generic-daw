use std::fmt::{Display, Formatter};

#[expect(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct dB(f32);

impl dB {
    #[must_use]
    pub fn from_amp(amp: f32) -> Self {
        Self(if amp < f32::EPSILON {
            f32::NEG_INFINITY
        } else {
            20.0 * amp.log10()
        })
    }

    #[must_use]
    pub fn to_amp(self) -> f32 {
        if self.0 == f32::NEG_INFINITY {
            0.0
        } else {
            10f32.powf(0.05 * self.0)
        }
    }
}

impl Display for dB {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2} dB", self.0)
    }
}
