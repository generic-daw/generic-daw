use generic_daw_core::{Meter, Position};
use iced::{
    Point, Rectangle, Renderer, Size, Theme, Transformation,
    advanced::{Renderer as _, renderer::Quad},
    widget::text::Shaping,
};
use std::sync::atomic::Ordering::Acquire;

mod arrangement;
mod arrangement_position;
mod arrangement_scale;
mod audio_clip;
mod bpm_input;
mod clipped;
mod knob;
mod peak_meter;
mod piano_roll;
mod redrawer;
mod track;
mod vsplit;

pub use arrangement::Arrangement;
pub use arrangement_position::ArrangementPosition;
pub use arrangement_scale::ArrangementScale;
pub use audio_clip::AudioClip;
pub use bpm_input::BpmInput;
pub use clipped::Clipped;
pub use knob::Knob;
pub use peak_meter::PeakMeter;
pub use redrawer::Redrawer;
pub use track::Track;
pub use vsplit::{Strategy, VSplit};

pub const LINE_HEIGHT: f32 = TEXT_HEIGHT * 1.3;
pub const TEXT_HEIGHT: f32 = 16.0;

pub const SWM: f32 = 60.0;

pub fn shaping_of(text: &str) -> Shaping {
    if text.is_ascii() {
        Shaping::Basic
    } else {
        Shaping::Advanced
    }
}

fn grid(
    renderer: &mut Renderer,
    bounds: Rectangle,
    theme: &Theme,
    meter: &Meter,
    position: ArrangementPosition,
    scale: ArrangementScale,
) {
    renderer.start_transformation(Transformation::translate(
        bounds.position().x,
        bounds.position().y,
    ));

    let numerator = meter.numerator.load(Acquire);
    let bpm = meter.bpm.load(Acquire);
    let sample_size = scale.x.exp2();

    let mut beat = Position::from_interleaved_samples_f(position.x, bpm, meter.sample_rate).ceil();

    let end_beat = beat
        + Position::from_interleaved_samples_f(bounds.width * sample_size, bpm, meter.sample_rate)
            .floor();

    while beat <= end_beat {
        let bar = beat.beat() / numerator as u32;
        let color = if scale.x >= 11.0 {
            if beat.beat() % numerator as u32 == 0 {
                if bar % 4 == 0 {
                    theme.extended_palette().background.strong.color
                } else {
                    theme.extended_palette().background.weak.color
                }
            } else {
                beat += Position::BEAT;
                continue;
            }
        } else if beat.beat() % numerator as u32 == 0 {
            theme.extended_palette().background.strong.color
        } else {
            theme.extended_palette().background.weak.color
        };

        let x = (beat.in_interleaved_samples_f(bpm, meter.sample_rate) - position.x) / sample_size;

        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(Point::new(x, 0.0), Size::new(1.0, bounds.height)),
                ..Quad::default()
            },
            color,
        );

        beat += Position::BEAT;
    }

    let offset = position.y.fract() * scale.y;

    let rows = (bounds.height / scale.y) as usize;

    for i in 0..=rows {
        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(
                    Point::new(0.0, (i as f32).mul_add(scale.y, -offset) - 0.5),
                    Size::new(bounds.width, 1.0),
                ),
                ..Quad::default()
            },
            theme.extended_palette().background.strong.color,
        );
    }

    renderer.end_transformation();
}
