use crate::{arrangement_view, arrangement_view::Tab, daw::Message};
use iced::keyboard::{self, key::Named};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandId {
	NewProject,
	OpenProject,
	SaveProject,
	SaveProjectAs,
	ExportProject,
	OpenSettings,
	ToggleFullscreen,
	TogglePlayback,
	StopPlayback,
	ToggleMidiRecording,
	FocusPlaylist,
	FocusMixer,
	FocusPianoRoll,
	MoveLeft,
	MoveRight,
	CycleTabForward,
	CycleTabBackward,
	SelectAll,
	UnselectAll,
	DuplicateSelection,
	DeleteSelection,
	CloseSettings,
	CloseClipInspector,
	ResetClipEdits,
	ToggleClipReverse,
}

#[derive(Clone, Copy, Debug)]
pub struct Context {
	pub config_open: bool,
	pub has_midi_clip: bool,
	pub tab: Tab,
	pub audio_clip_inspector_open: bool,
	pub armed_track: bool,
	pub midi_recording: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CommandSpec {
	pub id: CommandId,
	pub label: &'static str,
	pub shortcut: Option<Shortcut>,
	pub allow_repeat: bool,
	pub enabled: fn(Context) -> bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Shortcut {
	pub command: bool,
	pub shift: bool,
	pub alt: bool,
	pub key: ShortcutKey,
	pub display: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub enum ShortcutKey {
	Named(Named),
	Nameds(&'static [Named]),
	Latin(char),
}

const ALWAYS: fn(Context) -> bool = |_| true;
const WHEN_ARRANGEMENT_ACTIVE: fn(Context) -> bool = |ctx| !ctx.config_open;
const WHEN_PLAYLIST_HIDDEN: fn(Context) -> bool =
	|ctx| !ctx.config_open && ctx.tab != Tab::Playlist;
const WHEN_MIXER_HIDDEN: fn(Context) -> bool = |ctx| !ctx.config_open && ctx.tab != Tab::Mixer;
const WHEN_CONFIG_CLOSED_AND_MIDI_CLIP: fn(Context) -> bool =
	|ctx| !ctx.config_open && ctx.has_midi_clip && ctx.tab != Tab::PianoRoll;
const WHEN_CONFIG_OPEN: fn(Context) -> bool = |ctx| ctx.config_open;
const WHEN_CLIP_INSPECTOR_OPEN: fn(Context) -> bool =
	|ctx| !ctx.config_open && ctx.audio_clip_inspector_open;
const WHEN_MIDI_RECORDING_AVAILABLE: fn(Context) -> bool =
	|ctx| !ctx.config_open && (ctx.midi_recording || ctx.armed_track);
const WHEN_EDITABLE_ARRANGEMENT_TAB: fn(Context) -> bool =
	|ctx| !ctx.config_open && ctx.tab != Tab::Mixer;
const WHEN_SELECTION_CAN_CLEAR: fn(Context) -> bool =
	|ctx| !ctx.config_open && !ctx.audio_clip_inspector_open && ctx.tab != Tab::Mixer;

pub const COMMANDS: &[CommandSpec] = &[
	spec(
		CommandId::NewProject,
		"New Project",
		Some(shortcut(
			true,
			false,
			false,
			ShortcutKey::Latin('n'),
			"Cmd/Ctrl+N",
		)),
		false,
		ALWAYS,
	),
	spec(
		CommandId::OpenProject,
		"Open Project",
		Some(shortcut(
			true,
			false,
			false,
			ShortcutKey::Latin('o'),
			"Cmd/Ctrl+O",
		)),
		false,
		ALWAYS,
	),
	spec(
		CommandId::SaveProject,
		"Save Project",
		Some(shortcut(
			true,
			false,
			false,
			ShortcutKey::Latin('s'),
			"Cmd/Ctrl+S",
		)),
		false,
		ALWAYS,
	),
	spec(
		CommandId::SaveProjectAs,
		"Save Project As",
		Some(shortcut(
			true,
			true,
			false,
			ShortcutKey::Latin('s'),
			"Cmd/Ctrl+Shift+S",
		)),
		false,
		ALWAYS,
	),
	spec(
		CommandId::ExportProject,
		"Export Project",
		Some(shortcut(
			true,
			false,
			false,
			ShortcutKey::Latin('e'),
			"Cmd/Ctrl+E",
		)),
		false,
		ALWAYS,
	),
	spec(
		CommandId::OpenSettings,
		"Open Settings",
		None,
		false,
		|ctx| !ctx.config_open,
	),
	spec(
		CommandId::ToggleFullscreen,
		"Toggle Fullscreen",
		Some(shortcut(
			false,
			false,
			false,
			ShortcutKey::Named(Named::F11),
			"F11",
		)),
		false,
		ALWAYS,
	),
	spec(
		CommandId::TogglePlayback,
		"Play / Pause",
		Some(shortcut(
			false,
			false,
			false,
			ShortcutKey::Named(Named::Space),
			"Space",
		)),
		false,
		ALWAYS,
	),
	spec(CommandId::StopPlayback, "Stop", None, false, ALWAYS),
	spec(
		CommandId::ToggleMidiRecording,
		"Toggle MIDI Recording",
		None,
		false,
		WHEN_MIDI_RECORDING_AVAILABLE,
	),
	spec(
		CommandId::FocusPlaylist,
		"Show Playlist",
		None,
		false,
		WHEN_PLAYLIST_HIDDEN,
	),
	spec(
		CommandId::FocusMixer,
		"Show Mixer",
		None,
		false,
		WHEN_MIXER_HIDDEN,
	),
	spec(
		CommandId::FocusPianoRoll,
		"Show Piano Roll",
		None,
		false,
		WHEN_CONFIG_CLOSED_AND_MIDI_CLIP,
	),
	spec(
		CommandId::MoveLeft,
		"Step Left",
		Some(shortcut(
			false,
			false,
			false,
			ShortcutKey::Named(Named::ArrowLeft),
			"Left Arrow",
		)),
		true,
		WHEN_ARRANGEMENT_ACTIVE,
	),
	spec(
		CommandId::MoveRight,
		"Step Right",
		Some(shortcut(
			false,
			false,
			false,
			ShortcutKey::Named(Named::ArrowRight),
			"Right Arrow",
		)),
		true,
		WHEN_ARRANGEMENT_ACTIVE,
	),
	spec(
		CommandId::CycleTabForward,
		"Next Tab",
		Some(shortcut(
			false,
			false,
			false,
			ShortcutKey::Named(Named::Tab),
			"Tab",
		)),
		false,
		WHEN_ARRANGEMENT_ACTIVE,
	),
	spec(
		CommandId::CycleTabBackward,
		"Previous Tab",
		Some(shortcut(
			false,
			true,
			false,
			ShortcutKey::Named(Named::Tab),
			"Shift+Tab",
		)),
		false,
		WHEN_ARRANGEMENT_ACTIVE,
	),
	spec(
		CommandId::SelectAll,
		"Select All",
		Some(shortcut(
			true,
			false,
			false,
			ShortcutKey::Latin('a'),
			"Cmd/Ctrl+A",
		)),
		false,
		WHEN_EDITABLE_ARRANGEMENT_TAB,
	),
	spec(
		CommandId::CloseSettings,
		"Close Settings",
		Some(shortcut(
			false,
			false,
			false,
			ShortcutKey::Named(Named::Escape),
			"Esc",
		)),
		false,
		WHEN_CONFIG_OPEN,
	),
	spec(
		CommandId::CloseClipInspector,
		"Close Clip Inspector",
		Some(shortcut(
			false,
			false,
			false,
			ShortcutKey::Named(Named::Escape),
			"Esc",
		)),
		false,
		WHEN_CLIP_INSPECTOR_OPEN,
	),
	spec(
		CommandId::UnselectAll,
		"Clear Selection",
		Some(shortcut(
			false,
			false,
			false,
			ShortcutKey::Named(Named::Escape),
			"Esc",
		)),
		false,
		WHEN_SELECTION_CAN_CLEAR,
	),
	spec(
		CommandId::DuplicateSelection,
		"Duplicate Selection",
		Some(shortcut(
			true,
			false,
			false,
			ShortcutKey::Latin('d'),
			"Cmd/Ctrl+D",
		)),
		true,
		WHEN_EDITABLE_ARRANGEMENT_TAB,
	),
	spec(
		CommandId::DeleteSelection,
		"Delete Selection",
		Some(shortcut(
			false,
			false,
			false,
			ShortcutKey::Nameds(&[Named::Delete, Named::Backspace]),
			"Delete / Backspace",
		)),
		true,
		WHEN_ARRANGEMENT_ACTIVE,
	),
	spec(
		CommandId::ResetClipEdits,
		"Reset Clip Edits",
		None,
		false,
		WHEN_CLIP_INSPECTOR_OPEN,
	),
	spec(
		CommandId::ToggleClipReverse,
		"Toggle Clip Reverse",
		None,
		false,
		WHEN_CLIP_INSPECTOR_OPEN,
	),
];

const fn spec(
	id: CommandId,
	label: &'static str,
	shortcut: Option<Shortcut>,
	allow_repeat: bool,
	enabled: fn(Context) -> bool,
) -> CommandSpec {
	CommandSpec {
		id,
		label,
		shortcut,
		allow_repeat,
		enabled,
	}
}

const fn shortcut(
	command: bool,
	shift: bool,
	alt: bool,
	key: ShortcutKey,
	display: &'static str,
) -> Shortcut {
	Shortcut {
		command,
		shift,
		alt,
		key,
		display,
	}
}

pub fn dispatch(id: CommandId) -> Message {
	match id {
		CommandId::NewProject => Message::NewFile,
		CommandId::OpenProject => Message::OpenFileDialog,
		CommandId::SaveProject => Message::SaveFile,
		CommandId::SaveProjectAs => Message::SaveAsFileDialog,
		CommandId::ExportProject => Message::ExportFileDialog,
		CommandId::OpenSettings => Message::OpenConfigView,
		CommandId::ToggleFullscreen => Message::ToggleFullscreen,
		CommandId::TogglePlayback => {
			Message::Arrangement(arrangement_view::Message::TogglePlayback)
		}
		CommandId::StopPlayback => Message::Arrangement(arrangement_view::Message::Stop),
		CommandId::ToggleMidiRecording => {
			Message::Arrangement(arrangement_view::Message::ToggleRecord)
		}
		CommandId::FocusPlaylist => {
			Message::Arrangement(arrangement_view::Message::ChangedTab(Tab::Playlist))
		}
		CommandId::FocusMixer => {
			Message::Arrangement(arrangement_view::Message::ChangedTab(Tab::Mixer))
		}
		CommandId::FocusPianoRoll => {
			Message::Arrangement(arrangement_view::Message::ChangedTab(Tab::PianoRoll))
		}
		CommandId::MoveLeft => Message::Arrangement(arrangement_view::Message::ArrowLeft),
		CommandId::MoveRight => Message::Arrangement(arrangement_view::Message::ArrowRight),
		CommandId::CycleTabForward => {
			Message::Arrangement(arrangement_view::Message::CycleTabForwards)
		}
		CommandId::CycleTabBackward => {
			Message::Arrangement(arrangement_view::Message::CycleTabBackwards)
		}
		CommandId::SelectAll => Message::Arrangement(arrangement_view::Message::SelectAll),
		CommandId::UnselectAll => Message::Arrangement(arrangement_view::Message::UnselectAll),
		CommandId::DuplicateSelection => Message::Arrangement(arrangement_view::Message::Duplicate),
		CommandId::DeleteSelection => Message::Arrangement(arrangement_view::Message::Delete),
		CommandId::CloseSettings => Message::CloseConfigView,
		CommandId::CloseClipInspector => {
			Message::Arrangement(arrangement_view::Message::CloseAudioClipInspector)
		}
		CommandId::ResetClipEdits => {
			Message::Arrangement(arrangement_view::Message::AudioClipReset)
		}
		CommandId::ToggleClipReverse => {
			Message::Arrangement(arrangement_view::Message::AudioClipToggleReverse)
		}
	}
}

pub fn matching_shortcut(
	ctx: Context,
	key: &keyboard::Key,
	physical_key: keyboard::key::Physical,
	modifiers: keyboard::Modifiers,
	repeat: bool,
) -> Option<CommandId> {
	COMMANDS.iter().find_map(|command| {
		let shortcut = command.shortcut?;
		if (!command.allow_repeat && repeat) || !(command.enabled)(ctx) {
			return None;
		}
		if shortcut.command != modifiers.command()
			|| shortcut.shift != modifiers.shift()
			|| shortcut.alt != modifiers.alt()
		{
			return None;
		}

		let matches = match shortcut.key {
			ShortcutKey::Named(named) => key.as_ref() == keyboard::Key::Named(named),
			ShortcutKey::Nameds(nameds) => nameds
				.iter()
				.any(|&named| key.as_ref() == keyboard::Key::Named(named)),
			ShortcutKey::Latin(expected) => key
				.to_latin(physical_key)
				.is_some_and(|key| key == expected),
		};

		matches.then_some(command.id)
	})
}

pub fn available(ctx: Context) -> impl Iterator<Item = &'static CommandSpec> {
	COMMANDS
		.iter()
		.filter(move |command| (command.enabled)(ctx))
}

pub fn matches_query(command: &CommandSpec, query: &str) -> bool {
	let query = query.trim().to_ascii_lowercase();
	if query.is_empty() {
		return true;
	}

	command.label.to_ascii_lowercase().contains(&query)
		|| command
			.shortcut
			.is_some_and(|shortcut| shortcut.display.to_ascii_lowercase().contains(&query))
}

#[cfg(test)]
mod tests {
	use super::*;
	use iced::keyboard::key::{Code, Physical};

	fn playlist_context() -> Context {
		Context {
			config_open: false,
			has_midi_clip: false,
			tab: Tab::Playlist,
			audio_clip_inspector_open: false,
			armed_track: false,
			midi_recording: false,
		}
	}

	#[test]
	fn config_escape_closes_settings_before_selection() {
		let command = matching_shortcut(
			Context {
				config_open: true,
				..playlist_context()
			},
			&keyboard::Key::Named(Named::Escape),
			Physical::Code(Code::Escape),
			keyboard::Modifiers::default(),
			false,
		);

		assert_eq!(command, Some(CommandId::CloseSettings));
	}

	#[test]
	fn save_shortcut_dispatches_from_registry() {
		let command = matching_shortcut(
			playlist_context(),
			&keyboard::Key::Character("s".into()),
			Physical::Code(Code::KeyS),
			keyboard::Modifiers::COMMAND,
			false,
		);

		assert_eq!(command, Some(CommandId::SaveProject));
		assert!(matches!(dispatch(command.unwrap()), Message::SaveFile));
	}

	#[test]
	fn audio_clip_commands_follow_context() {
		let available = available(Context {
			audio_clip_inspector_open: true,
			..playlist_context()
		})
		.map(|command| command.id)
		.collect::<Vec<_>>();

		assert!(available.contains(&CommandId::CloseClipInspector));
		assert!(available.contains(&CommandId::ResetClipEdits));
		assert!(available.contains(&CommandId::ToggleClipReverse));
		assert!(!available.contains(&CommandId::CloseSettings));
	}
}
