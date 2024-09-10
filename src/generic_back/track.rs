pub mod audio_track;
pub mod midi_track;

use super::{position::Position, track_clip::ClipType};
use crate::generic_front::timeline::Message;
use audio_track::AudioTrack;
use iced::{
    advanced::{layout, mouse, renderer, widget, Layout, Widget},
    Length, Rectangle, Renderer, Size, Theme,
};
use midi_track::MidiTrack;
use std::sync::{Arc, RwLock};

pub enum TrackType {
    Audio(Arc<RwLock<AudioTrack>>),
    Midi(Arc<RwLock<MidiTrack>>),
}

impl TrackType {
    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        match self {
            Self::Audio(track) => track.read().unwrap().get_at_global_time(global_time),
            Self::Midi(track) => track.read().unwrap().get_at_global_time(global_time),
        }
    }

    pub fn get_global_end(&self) -> Position {
        match self {
            Self::Audio(track) => track.read().unwrap().get_global_end(),
            Self::Midi(track) => track.read().unwrap().get_global_end(),
        }
    }

    pub fn get_volume(&self) -> f32 {
        match self {
            Self::Audio(track) => track.read().unwrap().volume,
            Self::Midi(track) => track.read().unwrap().volume,
        }
    }

    pub fn set_volume(&self, volume: f32) {
        match self {
            Self::Audio(track) => track.write().unwrap().volume = volume,
            Self::Midi(track) => track.write().unwrap().volume = volume,
        }
    }

    pub fn push(&mut self, clip: ClipType) {
        match clip {
            ClipType::Audio(clip) => match self {
                Self::Audio(track) => track.write().unwrap().clips.write().unwrap().push(clip),
                Self::Midi(_) => panic!(),
            },
            ClipType::Midi(clip) => match self {
                Self::Midi(track) => track.write().unwrap().clips.write().unwrap().push(clip),
                Self::Audio(_) => panic!(),
            },
        }
    }
}

impl Widget<Message, Theme, Renderer> for TrackType {
    fn size(&self) -> Size<Length> {
        match self {
            Self::Audio(track) => track.size(),
            Self::Midi(track) => track.size(),
        }
    }

    fn layout(
        &self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        match self {
            Self::Audio(track) => track.layout(tree, renderer, limits),
            Self::Midi(track) => track.layout(tree, renderer, limits),
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        match self {
            Self::Audio(track) => {
                track.draw(tree, renderer, theme, style, layout, cursor, viewport);
            }
            Self::Midi(track) => {
                track.draw(tree, renderer, theme, style, layout, cursor, viewport);
            }
        }
    }
}
