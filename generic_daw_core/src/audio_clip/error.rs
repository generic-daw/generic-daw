use rubato::{ResampleError, ResamplerConstructionError};
use std::{
    error::Error,
    fmt::{Display, Formatter},
};
use symphonia::core::errors::Error as SymphoniaError;

#[derive(Debug)]
pub enum RubatoError {
    ResamplerConstructionError(ResamplerConstructionError),
    ResampleError(ResampleError),
}

impl Display for RubatoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResamplerConstructionError(err) => err.fmt(f),
            Self::ResampleError(err) => err.fmt(f),
        }
    }
}

impl Error for RubatoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::ResamplerConstructionError(err) => err,
            Self::ResampleError(err) => err,
        })
    }
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

impl Display for InterleavedAudioError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RubatoError(err) => err.fmt(f),
            Self::SymphoniaError(err) => err.fmt(f),
        }
    }
}

impl Error for InterleavedAudioError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::RubatoError(err) => err,
            Self::SymphoniaError(err) => err,
        })
    }
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
