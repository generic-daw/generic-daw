use iced::{
    advanced::{renderer::Quad, Renderer as _},
    Rectangle, Renderer, Size, Theme, Vector,
};

mod arrangement;
mod arrangement_position;
mod arrangement_scale;
mod knob;
mod track;
mod track_clip;
mod vsplit;

pub use arrangement::Arrangement;
pub use arrangement_position::ArrangementPosition;
pub use arrangement_scale::ArrangementScale;
pub use knob::Knob;
pub use track::Track;
pub use track_clip::TrackClip;
pub use vsplit::VSplit;

pub const LINE_HEIGHT: f32 = 21.0;

fn border(renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
    // I have no clue why we sometimes have to subtract one extra from the y coordinate
    // but it works so I'm not gonna touch it

    renderer.fill_quad(
        Quad {
            bounds: Rectangle::new(bounds.position(), Size::new(0.5, bounds.height)),
            ..Quad::default()
        },
        theme.extended_palette().secondary.weak.color,
    );

    renderer.fill_quad(
        Quad {
            bounds: Rectangle::new(
                bounds.position() + Vector::new(0.0, -1.0),
                Size::new(bounds.width, 0.5),
            ),
            ..Quad::default()
        },
        theme.extended_palette().secondary.weak.color,
    );

    renderer.fill_quad(
        Quad {
            bounds: Rectangle::new(
                bounds.position() + Vector::new(bounds.width - 1.0, 0.0),
                Size::new(0.5, bounds.height),
            ),
            ..Quad::default()
        },
        theme.extended_palette().secondary.weak.color,
    );

    renderer.fill_quad(
        Quad {
            bounds: Rectangle::new(
                bounds.position() + Vector::new(0.0, bounds.height - 2.0),
                Size::new(bounds.width, 0.5),
            ),
            ..Quad::default()
        },
        theme.extended_palette().secondary.weak.color,
    );
}
