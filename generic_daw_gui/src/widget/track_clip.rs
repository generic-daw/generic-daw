use super::{ArrangementPosition, ArrangementScale, LINE_HEIGHT};
use generic_daw_core::TrackClip as TrackClipInner;
use iced::{
    advanced::{
        graphics::{geometry::Renderer as _, Mesh},
        layout::{Limits, Node},
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::{tree, Tree},
        Clipboard, Layout, Renderer as _, Shell, Text, Widget,
    },
    alignment::{Horizontal, Vertical},
    event::Status,
    mouse::{Cursor, Interaction},
    widget::text::{LineHeight, Shaping, Wrapping},
    window, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
};
use iced_wgpu::{
    geometry::Cache,
    graphics::cache::{Cached as _, Group},
    Geometry,
};
use std::{cell::RefCell, cmp::min_by, sync::Arc};

pub mod audio_clip;
pub mod track_clip_ext;

pub use track_clip_ext::TrackClipExt;

#[derive(Default)]
struct State {
    /// the mesh cache
    cache: RefCell<Option<Cache>>,
    /// the theme from the last draw
    last_theme: RefCell<Option<Theme>>,
    /// the position from the last draw
    last_position: ArrangementPosition,
    /// the scale from the last draw
    last_scale: ArrangementScale,
}

#[derive(Clone)]
pub struct TrackClip {
    inner: Arc<TrackClipInner>,
    /// the position of the top left corner of the arrangement viewport
    position: ArrangementPosition,
    /// the scale of the timeline viewport
    scale: ArrangementScale,
    // whether the clip is in an enabled track
    enabled: bool,
}

impl<Message> Widget<Message, Theme, Renderer> for TrackClip {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

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
                / self.scale.x.exp2(),
            limits.max().height,
        ))
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> Status {
        if let Event::Window(window::Event::RedrawRequested(..)) = event {
            let state = tree.state.downcast_mut::<State>();

            if state.last_position != self.position {
                state.last_position = self.position;
                state.cache.take();
            }

            if state.last_scale != self.scale {
                state.last_scale = self.scale;
                state.cache.take();
            }
        }

        Status::Ignored
    }

    fn draw(
        &self,
        tree: &Tree,
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

        // the bounds of the text part of the clip
        let mut upper_bounds = bounds;
        upper_bounds.height = min_by(upper_bounds.height, LINE_HEIGHT, f32::total_cmp);

        let color = if self.enabled {
            theme.extended_palette().primary.weak.color
        } else {
            theme.extended_palette().secondary.weak.color
        };

        // the opaque background of the text
        let text_background = Quad {
            bounds: upper_bounds,
            ..Quad::default()
        };
        renderer.fill_quad(text_background, color);

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
            upper_bounds.position() + Vector::new(4.0, -1.0),
            theme.extended_palette().secondary.base.text,
            upper_bounds,
        );

        if bounds.height == upper_bounds.height {
            return;
        }

        // the bounds of the waveform part of the clip
        let mut lower_bounds = bounds;
        lower_bounds.height -= upper_bounds.height;
        lower_bounds.y += upper_bounds.height;

        // https://github.com/iced-rs/iced/issues/2700
        if lower_bounds.height < 1.0 {
            return;
        }

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: lower_bounds,
            ..Quad::default()
        };
        renderer.fill_quad(clip_background, color.scale_alpha(0.25));

        let state = tree.state.downcast_ref::<State>();

        // clear the mesh cache if the theme has changed
        if state
            .last_theme
            .borrow()
            .as_ref()
            .is_none_or(|last_theme| last_theme != theme)
        {
            state.cache.take();
            state.last_theme.borrow_mut().replace(theme.clone());
        }

        // fill the mesh cache if it's cleared
        if state.cache.borrow().is_none() {
            let mesh = self
                .inner
                .mesh(theme, bounds.size(), self.position, self.scale);

            state.cache.borrow_mut().replace(
                Geometry::Live {
                    meshes: vec![mesh],
                    images: Vec::new(),
                    text: Vec::new(),
                }
                .cache(Group::unique(), None),
            );
        }

        // draw the mesh
        renderer.with_layer(lower_bounds, |renderer| {
            renderer.with_translation(Vector::new(upper_bounds.x, layout.bounds().y), |renderer| {
                renderer.draw_geometry(Geometry::load(state.cache.borrow().as_ref().unwrap()));
            });
        });
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
    pub fn new(
        inner: Arc<TrackClipInner>,
        position: ArrangementPosition,
        scale: ArrangementScale,
        enabled: bool,
    ) -> Self {
        Self {
            inner,
            position,
            scale,
            enabled,
        }
    }
}

impl TrackClipExt for TrackClipInner {
    fn mesh(
        &self,
        theme: &Theme,
        size: Size,
        position: ArrangementPosition,
        scale: ArrangementScale,
    ) -> Mesh {
        match self {
            Self::Audio(audio) => audio.mesh(theme, size, position, scale),
            Self::Midi(_) => unreachable!(),
        }
    }
}
