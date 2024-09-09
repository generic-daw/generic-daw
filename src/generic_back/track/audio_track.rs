use super::Track;
use crate::{
    generic_back::{meter::Meter, position::Position, track_clip::audio_clip::AudioClip},
    generic_front::drawable::{Drawable, TimelinePosition, TimelineScale},
};
use iced::{widget::canvas::Frame, Theme};
use std::sync::{atomic::Ordering::SeqCst, RwLock};

pub struct AudioTrack {
    pub clips: RwLock<Vec<AudioClip>>,
    volume: f32,
}

impl Default for AudioTrack {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioTrack {
    pub const fn new() -> Self {
        Self {
            clips: RwLock::new(Vec::new()),
            volume: 1.0,
        }
    }
}

impl Track for AudioTrack {
    fn get_at_global_time(&self, global_time: usize, meter: &Meter) -> f32 {
        if !meter.playing.load(SeqCst) {
            return 0.0;
        }

        self.clips
            .read()
            .unwrap()
            .iter()
            .map(|clip| clip.get_at_global_time(global_time, meter))
            .sum::<f32>()
            * self.volume
    }

    fn get_global_end(&self) -> Position {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(AudioClip::get_global_end)
            .max()
            .unwrap_or(Position::new(0, 0))
    }

    fn get_volume(&self) -> f32 {
        self.volume
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }
}

impl Drawable for AudioTrack {
    fn draw(
        &self,
        frame: &mut Frame,
        scale: TimelineScale,
        position: &TimelinePosition,
        meter: &Meter,
        theme: &Theme,
    ) {
        let path = iced::widget::canvas::Path::new(|path| {
            let y = (position.y + 1.0) * scale.y;
            path.line_to(iced::Point::new(0.0, y));
            path.line_to(iced::Point::new(frame.width(), y));
        });
        frame.stroke(
            &path,
            iced::widget::canvas::Stroke::default()
                .with_color(theme.extended_palette().secondary.base.color),
        );

        self.clips.read().unwrap().iter().for_each(|track| {
            track.draw(frame, scale, position, meter, theme);
        });
    }
}
