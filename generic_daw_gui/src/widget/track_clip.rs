use super::{ArrangementPosition, ArrangementScale, LINE_HEIGHT};
use generic_daw_core::TrackClip as TrackClipInner;
use iced::{
    advanced::{
        graphics::Mesh,
        layout::{Limits, Node},
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::Tree,
        Layout, Renderer as _, Text, Widget,
    },
    alignment::{Horizontal, Vertical},
    mouse::{Cursor, Interaction},
    widget::text::{LineHeight, Shaping, Wrapping},
    Length, Rectangle, Renderer, Size, Theme, Vector,
};
use std::{cmp::max_by, rc::Rc, sync::Arc};

pub mod audio_clip;
pub mod track_clip_ext;

pub use track_clip_ext::TrackClipExt;

#[derive(Clone)]
pub struct TrackClip {
    inner: Arc<TrackClipInner>,
    /// information about the scale of the timeline viewport
    scale: Rc<ArrangementScale>,
}

impl<Message> Widget<Message, Theme, Renderer> for TrackClip {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Fill,
        }
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        let meter = self.inner.meter();

        Node::new(Size::new(
            (self.inner.get_global_end().in_interleaved_samples(meter)
                - self.inner.get_global_start().in_interleaved_samples(meter)) as f32
                / self.scale.x.get().exp2(),
            limits.max().height,
        ))
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let Some(bounds) = layout.bounds().intersection(viewport) else {
            return;
        };

        // https://github.com/iced-rs/iced/issues/2700
        if bounds.height < 1.0 {
            return;
        }

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: Rectangle::new(
                bounds.position(),
                Size::new(bounds.width, max_by(0.0, bounds.height, f32::total_cmp)),
            ),
            ..Quad::default()
        };

        renderer.fill_quad(
            clip_background,
            theme
                .extended_palette()
                .primary
                .weak
                .color
                .scale_alpha(0.25),
        );

        // height of the clip, excluding the text, clipped off by the top of the arrangement
        let clip_height = max_by(0.0, LINE_HEIGHT - bounds.height, f32::total_cmp);

        // the opaque background of the text
        let text_background = Quad {
            bounds: Rectangle::new(
                bounds.position() + Vector::new(0.0, -clip_height),
                Size::new(bounds.width, LINE_HEIGHT),
            ),
            ..Quad::default()
        };

        renderer.fill_quad(text_background, theme.extended_palette().primary.weak.color);

        // the text containing the name of the sample
        let text = Text {
            content: self.inner.get_name(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: renderer.default_size(),
            line_height: LineHeight::default(),
            font: renderer.default_font(),
            horizontal_alignment: Horizontal::Left,
            vertical_alignment: Vertical::Top,
            shaping: Shaping::default(),
            wrapping: Wrapping::default(),
        };

        renderer.fill_text(
            text,
            bounds.position() + Vector::new(3.0, clip_height - 1.0),
            theme.extended_palette().secondary.base.text,
            bounds,
        );
    }

    fn mouse_interaction(
        &self,
        _state: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> Interaction {
        let bounds = layout.bounds();

        if let Some(cursor) = cursor.position_in(bounds) {
            if cursor.x < 10.0 || bounds.width - cursor.x < 10.0 {
                return Interaction::ResizingHorizontally;
            }

            return Interaction::Grab;
        }

        Interaction::default()
    }
}

impl TrackClip {
    pub fn new(inner: Arc<TrackClipInner>, scale: Rc<ArrangementScale>) -> Self {
        Self { inner, scale }
    }
}

impl TrackClipExt for TrackClipInner {
    fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        viewport: Rectangle,
        position: &ArrangementPosition,
        scale: &ArrangementScale,
    ) -> Option<Mesh> {
        match self {
            Self::Audio(audio) => audio.meshes(theme, bounds, viewport, position, scale),
            Self::Midi(_) => None,
        }
    }
}
