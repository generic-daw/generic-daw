mod audio_clip;

use crate::{
    generic_back::TrackClip as TrackClipInner,
    generic_front::{TimelinePosition, TimelineScale},
};
use iced::{
    advanced::{
        graphics::Mesh,
        layout::{Limits, Node},
        mouse,
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::Tree,
        Layout, Renderer as _, Text, Widget,
    },
    alignment::{Horizontal, Vertical},
    mouse::Interaction,
    widget::text::{LineHeight, Shaping, Wrapping},
    Element, Font, Length, Pixels, Point, Rectangle, Renderer, Size, Theme, Vector,
};
use std::{
    cmp::max_by,
    fmt::{Debug, Formatter},
    rc::Rc,
    sync::Arc,
};

#[derive(Clone)]
pub struct TrackClip {
    inner: Arc<TrackClipInner>,
    /// information about the scale of the timeline viewport
    scale: Rc<TimelineScale>,
}

impl Debug for TrackClip {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("").field(&self.inner).finish_non_exhaustive()
    }
}

impl<Message> Widget<Message, Theme, Renderer> for TrackClip {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Fill,
        }
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        let meter = match &*self.inner {
            TrackClipInner::Audio(clip) => &clip.meter,
            TrackClipInner::Midi(clip) => &clip.meter,
        };

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
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let Some(bounds) = layout.bounds().intersection(viewport) else {
            return;
        };

        // TODO fix this iced bug
        // https://github.com/iced-rs/iced/issues/2700
        if bounds.height < 1.0 {
            return;
        }

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: Rectangle::new(
                Point::new(0.0, 0.0),
                Size::new(
                    bounds.width,
                    max_by(0.0, bounds.height, |a, b| a.partial_cmp(b).unwrap()),
                ),
            ),
            ..Quad::default()
        };

        // height of the clip, excluding the text, clipped off by the top of the arrangement
        let clip_height = max_by(0.0, 18.0 - bounds.height, |a, b| a.partial_cmp(b).unwrap());

        // the opaque background of the text
        let text_background = Quad {
            bounds: Rectangle::new(Point::new(0.0, -clip_height), Size::new(bounds.width, 18.0)),
            ..Quad::default()
        };

        // the text containing the name of the sample
        let text = Text {
            content: self.inner.get_name(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: Pixels(12.0),
            line_height: LineHeight::default(),
            font: Font::default(),
            horizontal_alignment: Horizontal::Left,
            vertical_alignment: Vertical::Top,
            shaping: Shaping::default(),
            wrapping: Wrapping::default(),
        };

        renderer.with_layer(bounds, |renderer| {
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
                    Point::new(3.0, 2.0 - clip_height),
                    theme.extended_palette().secondary.base.text,
                    Rectangle::INFINITE,
                );
            });
        });
    }

    fn mouse_interaction(
        &self,
        _state: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
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
    pub fn new(inner: Arc<TrackClipInner>, scale: Rc<TimelineScale>) -> Self {
        Self { inner, scale }
    }

    pub fn is(&self, other: &Arc<TrackClipInner>) -> bool {
        Arc::ptr_eq(&self.inner, other)
    }
}

impl TrackClipInner {
    pub fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        viewport: Rectangle,
        position: &TimelinePosition,
        scale: &TimelineScale,
    ) -> Option<Mesh> {
        match self {
            Self::Audio(audio) => audio.meshes(theme, bounds, viewport, position, scale),
            Self::Midi(_) => None,
        }
    }
}

impl<Message> From<TrackClip> for Element<'_, Message, Theme, Renderer> {
    fn from(track: TrackClip) -> Self {
        Self::new(track)
    }
}
