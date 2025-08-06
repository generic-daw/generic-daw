use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
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
	#[default]
	CatppuccinFrappe,
	CatppuccinMacchiato,
	CatppuccinMocha,
	TokyoNight,
	TokyoNightStorm,
	TokyoNightLight,
	KanagawaWave,
	KanagawaLotus,
	Moonfly,
	Nightfly,
	Oxocarbon,
	Ferra,
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
			Theme::KanagawaLotus => Self::KanagawaLotus,
			Theme::Moonfly => Self::Moonfly,
			Theme::Nightfly => Self::Nightfly,
			Theme::Oxocarbon => Self::Oxocarbon,
			Theme::Ferra => Self::Ferra,
		}
	}
}

impl TryFrom<iced::Theme> for Theme {
	type Error = ();

	fn try_from(value: iced::Theme) -> Result<Self, Self::Error> {
		match value {
			iced::Theme::Light => Ok(Self::Light),
			iced::Theme::Dark => Ok(Self::Dark),
			iced::Theme::Dracula => Ok(Self::Dracula),
			iced::Theme::Nord => Ok(Self::Nord),
			iced::Theme::SolarizedLight => Ok(Self::SolarizedLight),
			iced::Theme::SolarizedDark => Ok(Self::SolarizedDark),
			iced::Theme::GruvboxLight => Ok(Self::GruvboxLight),
			iced::Theme::GruvboxDark => Ok(Self::GruvboxDark),
			iced::Theme::CatppuccinLatte => Ok(Self::CatppuccinLatte),
			iced::Theme::CatppuccinFrappe => Ok(Self::CatppuccinFrappe),
			iced::Theme::CatppuccinMacchiato => Ok(Self::CatppuccinMacchiato),
			iced::Theme::CatppuccinMocha => Ok(Self::CatppuccinMocha),
			iced::Theme::TokyoNight => Ok(Self::TokyoNight),
			iced::Theme::TokyoNightStorm => Ok(Self::TokyoNightStorm),
			iced::Theme::TokyoNightLight => Ok(Self::TokyoNightLight),
			iced::Theme::KanagawaWave => Ok(Self::KanagawaWave),
			iced::Theme::KanagawaLotus => Ok(Self::KanagawaLotus),
			iced::Theme::Moonfly => Ok(Self::Moonfly),
			iced::Theme::Nightfly => Ok(Self::Nightfly),
			iced::Theme::Oxocarbon => Ok(Self::Oxocarbon),
			iced::Theme::Ferra => Ok(Self::Ferra),
			_ => Err(()),
		}
	}
}
