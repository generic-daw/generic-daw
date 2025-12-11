use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use utils::variants;

variants! {
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Theme {
	Light,
	Dark,
	Dracula,
	Nord,
	SolarizedLight,
	SolarizedDark,
	GruvboxLight,
	GruvboxDark,
	CatppuccinLatte,
	CatppuccinFrappe,
	CatppuccinMacchiato,
	CatppuccinMocha,
	TokyoNight,
	TokyoNightStorm,
	TokyoNightLight,
	KanagawaWave,
	KanagawaDragon,
	KanagawaLotus,
	Moonfly,
	Nightfly,
	Oxocarbon,
	Ferra,
}
}

impl From<Theme> for iced::Theme {
	fn from(value: Theme) -> Self {
		match value {
			Theme::Light => Self::Light,
			Theme::Dark => Self::Dark,
			Theme::Dracula => Self::Dracula,
			Theme::Nord => Self::Nord,
			Theme::SolarizedLight => Self::SolarizedLight,
			Theme::SolarizedDark => Self::SolarizedDark,
			Theme::GruvboxLight => Self::GruvboxLight,
			Theme::GruvboxDark => Self::GruvboxDark,
			Theme::CatppuccinLatte => Self::CatppuccinLatte,
			Theme::CatppuccinFrappe => Self::CatppuccinFrappe,
			Theme::CatppuccinMacchiato => Self::CatppuccinMacchiato,
			Theme::CatppuccinMocha => Self::CatppuccinMocha,
			Theme::TokyoNight => Self::TokyoNight,
			Theme::TokyoNightStorm => Self::TokyoNightStorm,
			Theme::TokyoNightLight => Self::TokyoNightLight,
			Theme::KanagawaWave => Self::KanagawaWave,
			Theme::KanagawaDragon => Self::KanagawaDragon,
			Theme::KanagawaLotus => Self::KanagawaLotus,
			Theme::Moonfly => Self::Moonfly,
			Theme::Nightfly => Self::Nightfly,
			Theme::Oxocarbon => Self::Oxocarbon,
			Theme::Ferra => Self::Ferra,
		}
	}
}

impl Display for Theme {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		iced::Theme::from(*self).fmt(f)
	}
}
