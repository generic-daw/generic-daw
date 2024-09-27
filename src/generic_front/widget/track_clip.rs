use crate::{generic_back::TrackClip, generic_front::ArrangementState};
use iced::{
    advanced::{graphics::Mesh, renderer::Quad, text::Renderer as _, Renderer as _, Text},
    alignment::{Horizontal, Vertical},
    widget::text::{LineHeight, Shaping, Wrapping},
    Font, Pixels, Point, Rectangle, Renderer, Size, Theme, Vector,
};
use std::cmp::max_by;

impl TrackClip {
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        bounds: Rectangle,
        arrangement_bounds: Rectangle,
    ) {
        // how many pixels of the top of the clip are clipped off by the top of the arrangement
        let hidden = max_by(0.0, arrangement_bounds.y - bounds.y, |a, b| {
            a.partial_cmp(b).unwrap()
        });

        // the part of the audio clip that is visible
        let clip_bounds = Rectangle::new(
            Point::new(0.0, hidden),
            bounds.intersection(&arrangement_bounds).unwrap().size(),
        );

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: Rectangle::new(
                Point::new(0.0, hidden),
                Size::new(
                    clip_bounds.width,
                    max_by(0.0, clip_bounds.height, |a, b| a.partial_cmp(b).unwrap()),
                ),
            ),
            ..Quad::default()
        };

        // height of the clip, excluding the text, clipped off by the top of the arrangement
        let clip_height = max_by(0.0, 18.0 - hidden, |a, b| a.partial_cmp(b).unwrap());

        // the opaque background of the text
        let text_background = Quad {
            bounds: Rectangle::new(
                Point::new(0.0, hidden),
                Size::new(clip_bounds.width, clip_height),
            ),
            ..Quad::default()
        };

        // the text containing the name of the sample
        let text = Text {
            content: self.get_name(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: Pixels(12.0),
            line_height: LineHeight::default(),
            font: Font::default(),
            horizontal_alignment: Horizontal::Left,
            vertical_alignment: Vertical::Top,
            shaping: Shaping::default(),
            wrapping: Wrapping::default(),
        };

        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
            renderer.fill_quad(
                clip_background,
                theme
                    .extended_palette()
                    .primary
                    .weak
                    .color
                    .scale_alpha(0.25),
            );

            renderer.fill_quad(text_background, theme.extended_palette().primary.weak.color);

            renderer.fill_text(
                text,
                Point::new(2.0, 2.0),
                theme.extended_palette().secondary.base.text,
                clip_bounds,
            );
        });
    }

    pub fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        arrangement_bounds: Rectangle,
        state: &ArrangementState,
    ) -> Option<Mesh> {
        match self {
            Self::Audio(audio) => audio.meshes(theme, bounds, arrangement_bounds, state),
            Self::Midi(_) => None,
        }
    }
}
