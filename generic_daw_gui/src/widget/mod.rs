use generic_daw_core::{Meter, Position};
use generic_daw_utils::Vec2;
use iced::{
    Point, Rectangle, Renderer, Size, Theme, Transformation,
    advanced::{Renderer as _, Shell, renderer::Quad},
    keyboard::Modifiers,
    mouse::ScrollDelta,
    widget::text::Shaping,
};
use std::sync::atomic::Ordering::Acquire;

mod arrangement;
mod audio_clip;
mod bpm_input;
mod clipped;
mod knob;
mod midi_clip;
mod peak_meter;
mod piano_roll;
mod redrawer;
mod track;
mod vsplit;

pub use arrangement::Arrangement;
pub use audio_clip::AudioClip;
pub use bpm_input::BpmInput;
pub use clipped::Clipped;
pub use knob::Knob;
pub use midi_clip::MidiClip;
pub use peak_meter::PeakMeter;
pub use piano_roll::PianoRoll;
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
    position: Vec2,
    scale: Vec2,
) {
    renderer.start_transformation(Transformation::translate(bounds.x, bounds.y));

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

    let rows = (bounds.height / scale.y) as usize + 1;

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

fn wheel_scrolled<Message>(
    delta: &ScrollDelta,
    modifiers: Modifiers,
    cursor: Point,
    scale: Vec2,
    shell: &mut Shell<'_, Message>,
    position_scale_delta: fn(Vec2, Vec2) -> Message,
) {
    let (mut x, mut y) = match *delta {
        ScrollDelta::Pixels { x, y } => (-x, -y),
        ScrollDelta::Lines { x, y } => (-x * SWM, -y * SWM),
    };

    match (modifiers.control(), modifiers.shift(), modifiers.alt()) {
        (false, false, false) => {
            x *= scale.x.exp2();
            y /= scale.y;

            shell.publish((position_scale_delta)(Vec2::new(x, y), Vec2::ZERO));
            shell.capture_event();
        }
        (true, false, false) => {
            x = y / 128.0;

            let mut x_pos = scale.x.exp2() - (scale.x + x).exp2();
            x_pos *= cursor.x;

            shell.publish((position_scale_delta)(
                Vec2::new(x_pos, 0.0),
                Vec2::new(x, 0.0),
            ));
            shell.capture_event();
        }
        (false, true, false) => {
            y *= 4.0 * scale.x.exp2();

            shell.publish((position_scale_delta)(Vec2::new(y, 0.0), Vec2::ZERO));
            shell.capture_event();
        }
        (false, false, true) => {
            y /= -8.0;

            let y_pos = ((cursor.y - LINE_HEIGHT) * y) / (scale.y.powi(2));

            shell.publish((position_scale_delta)(
                Vec2::new(0.0, y_pos),
                Vec2::new(0.0, y),
            ));
            shell.capture_event();
        }
        _ => {}
    }
}

fn get_time(x: f32, modifiers: Modifiers, meter: &Meter, position: Vec2, scale: Vec2) -> Position {
    let time = x.mul_add(scale.x.exp2(), position.x);
    let mut time =
        Position::from_interleaved_samples_f(time, meter.bpm.load(Acquire), meter.sample_rate);

    if !modifiers.alt() {
        time = time.snap(scale.x, meter.numerator.load(Acquire));
    }

    time
}
