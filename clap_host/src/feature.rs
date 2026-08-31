use clack_host::plugin::features::{
	AMBISONIC, ANALYZER, AUDIO_EFFECT, CHORUS, COMPRESSOR, DEESSER, DELAY, DISTORTION,
	DRUM_MACHINE, EQUALIZER, FILTER, FLANGER, FREQUENCY_SHIFTER, GLITCH, GRANULAR, INSTRUMENT,
	LIMITER, MASTERING, MIXING, MONO, MULTI_EFFECTS, NOTE_EFFECT, PHASE_VOCODER, PHASER,
	PITCH_CORRECTION, PITCH_SHIFTER, RESTORATION, REVERB, SAMPLER, STEREO, SURROUND, SYNTHESIZER,
	TRANSIENT_SHAPER, TREMOLO, UTILITY,
};
use std::{
	ffi::CStr,
	fmt::{Display, Formatter},
};
use utils::variants;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Feature {
	Category(Category),
	Subcategory(Subcategory),
	Capability(Capability),
}

impl Feature {
	#[must_use]
	pub fn parse(value: &CStr) -> Option<Self> {
		macro_rules! parse {
			($($cond:expr => $ret:expr),* $(,)?) => {
				$(
					if value == $cond {
						Some($ret)
					} else
				)* {
					None
				}
			}
		}

		parse! {
			INSTRUMENT => Self::Category(Category::Instrument),
			AUDIO_EFFECT => Self::Category(Category::AudioEffect),
			NOTE_EFFECT => Self::Category(Category::NoteEffect),
			c"note-detector" => Self::Category(Category::NoteDetector),
			ANALYZER => Self::Category(Category::Analyzer),

			SYNTHESIZER => Self::Subcategory(Subcategory::Synthesizer),
			SAMPLER => Self::Subcategory(Subcategory::Sampler),
			DRUM_MACHINE => Self::Subcategory(Subcategory::DrumMachine),
			FILTER => Self::Subcategory(Subcategory::Filter),
			PHASER => Self::Subcategory(Subcategory::Phaser),
			EQUALIZER => Self::Subcategory(Subcategory::Equalizer),
			DEESSER => Self::Subcategory(Subcategory::DeEsser),
			PHASE_VOCODER => Self::Subcategory(Subcategory::PhaseVocoder),
			GRANULAR => Self::Subcategory(Subcategory::Granular),
			FREQUENCY_SHIFTER => Self::Subcategory(Subcategory::FrequencyShifter),
			PITCH_SHIFTER => Self::Subcategory(Subcategory::PitchShifter),
			DISTORTION => Self::Subcategory(Subcategory::Distortion),
			TRANSIENT_SHAPER => Self::Subcategory(Subcategory::TransientShaper),
			COMPRESSOR => Self::Subcategory(Subcategory::Compressor),
			c"expander" => Self::Subcategory(Subcategory::Expander),
			c"gate" => Self::Subcategory(Subcategory::Gate),
			LIMITER => Self::Subcategory(Subcategory::Limiter),
			FLANGER => Self::Subcategory(Subcategory::Flanger),
			CHORUS => Self::Subcategory(Subcategory::Chorus),
			DELAY => Self::Subcategory(Subcategory::Delay),
			REVERB => Self::Subcategory(Subcategory::Reverb),
			TREMOLO => Self::Subcategory(Subcategory::Tremolo),
			GLITCH => Self::Subcategory(Subcategory::Glitch),
			UTILITY => Self::Subcategory(Subcategory::Utility),
			PITCH_CORRECTION => Self::Subcategory(Subcategory::PitchCorrection),
			RESTORATION => Self::Subcategory(Subcategory::Restoration),
			MULTI_EFFECTS => Self::Subcategory(Subcategory::MultiEffects),
			MIXING => Self::Subcategory(Subcategory::Mixing),
			MASTERING => Self::Subcategory(Subcategory::Mastering),

			MONO => Self::Capability(Capability::Mono),
			STEREO => Self::Capability(Capability::Stereo),
			SURROUND => Self::Capability(Capability::Surround),
			AMBISONIC => Self::Capability(Capability::Ambisonic),
		}
	}
}

impl Display for Feature {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Category(category) => category.fmt(f),
			Self::Subcategory(subcategory) => subcategory.fmt(f),
			Self::Capability(capability) => capability.fmt(f),
		}
	}
}

variants! {
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Category {
	Instrument,
	AudioEffect,
	NoteEffect,
	NoteDetector,
	Analyzer,
}
}

impl Display for Category {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::Instrument => "Instrument",
			Self::AudioEffect => "Audio Effect",
			Self::NoteEffect => "Note Effect",
			Self::NoteDetector => "Note Detector",
			Self::Analyzer => "Analyzer",
		})
	}
}

variants! {
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Subcategory {
	Synthesizer,
	Sampler,
	DrumMachine,
	Filter,
	Phaser,
	Equalizer,
	DeEsser,
	PhaseVocoder,
	Granular,
	FrequencyShifter,
	PitchShifter,
	Distortion,
	TransientShaper,
	Compressor,
	Expander,
	Gate,
	Limiter,
	Flanger,
	Chorus,
	Delay,
	Reverb,
	Tremolo,
	Glitch,
	Utility,
	PitchCorrection,
	Restoration,
	MultiEffects,
	Mixing,
	Mastering,
}
}

impl Display for Subcategory {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::Synthesizer => "Synthesizer",
			Self::Sampler => "Sampler",
			Self::DrumMachine => "Drum Machine",
			Self::Filter => "Filter",
			Self::Phaser => "Phaser",
			Self::Equalizer => "Equalizer",
			Self::DeEsser => "De-Esser",
			Self::PhaseVocoder => "Phase Vocoder",
			Self::Granular => "Granular",
			Self::FrequencyShifter => "Frequency Shifter",
			Self::PitchShifter => "Pitch Shifter",
			Self::Distortion => "Distortion",
			Self::TransientShaper => "Transient Shaper",
			Self::Compressor => "Compressor",
			Self::Expander => "Expander",
			Self::Gate => "Gate",
			Self::Limiter => "Limiter",
			Self::Flanger => "Flanger",
			Self::Chorus => "Chorus",
			Self::Delay => "Delay",
			Self::Reverb => "Reverb",
			Self::Tremolo => "Tremolo",
			Self::Glitch => "Glitch",
			Self::Utility => "Utility",
			Self::PitchCorrection => "Pitch Correction",
			Self::Restoration => "Restoration",
			Self::MultiEffects => "Multi Effects",
			Self::Mixing => "Mixing",
			Self::Mastering => "Mastering",
		})
	}
}

variants! {
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Capability {
	Mono,
	Stereo,
	Surround,
	Ambisonic,
}
}

impl Display for Capability {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::Mono => "Mono",
			Self::Stereo => "Stereo",
			Self::Surround => "Surround",
			Self::Ambisonic => "Ambisonic",
		})
	}
}
