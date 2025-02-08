use rubato::{ResampleError, ResamplerConstructionError};
use symphonia::core::errors::Error as SymphoniaError;

#[derive(Debug)]
pub enum RubatoError {
    ResamplerConstructionError(ResamplerConstructionError),
    ResampleError(ResampleError),
}

impl From<ResamplerConstructionError> for RubatoError {
    fn from(value: ResamplerConstructionError) -> Self {
        Self::ResamplerConstructionError(value)
    }
}

impl From<ResampleError> for RubatoError {
    fn from(value: ResampleError) -> Self {
        Self::ResampleError(value)
    }
}

#[derive(Debug)]
pub enum InterleavedAudioError {
    RubatoError(RubatoError),
    SymphoniaError(SymphoniaError),
}

impl From<RubatoError> for InterleavedAudioError {
    fn from(value: RubatoError) -> Self {
        Self::RubatoError(value)
    }
}

impl From<SymphoniaError> for InterleavedAudioError {
    fn from(value: SymphoniaError) -> Self {
        Self::SymphoniaError(value)
    }
}
