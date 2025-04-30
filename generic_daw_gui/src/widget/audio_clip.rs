use super::{LINE_HEIGHT, Vec2, shaping_of, waveform};
use generic_daw_core::AudioClip as AudioClipInner;
use iced::{
    Element, Event, Fill, Length, Point, Rectangle, Renderer, Shrink, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Text, Widget,
        graphics::geometry::Renderer as _,
        layout::{Limits, Node},
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::{Tree, tree},
    },
    alignment::Vertical,
    mouse::{self, Cursor, Interaction},
    padding,
    widget::text::{Alignment, LineHeight, Shaping, Wrapping},
    window,
};
use iced_wgpu::{
    Geometry,
    geometry::Cache,
    graphics::cache::{Cached as _, Group},
};
use std::{cell::RefCell, sync::atomic::Ordering::Acquire};

#[derive(Default)]
struct State {
    cache: RefCell<Option<Cache>>,
    shaping: Shaping,
    interaction: Interaction,
    last_bounds: Rectangle,
    last_viewport: Rectangle,
    last_theme: RefCell<Option<Theme>>,
    last_addr: usize,
}

impl State {
    fn new(text: &str) -> Self {
        Self {
            shaping: shaping_of(text),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug)]
pub struct AudioClip<'a> {
    inner: &'a AudioClipInner,
    /// the position of the top left corner of the arrangement viewport
    position: &'a Vec2,
    /// the scale of the arrangement viewport
    scale: &'a Vec2,
    /// whether the clip is in an enabled track
    enabled: bool,
}

impl<Message> Widget<Message, Theme, Renderer> for AudioClip<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(&self.inner.audio.name))
    }

    fn size(&self) -> Size<Length> {
        Size::new(Shrink, Fill)
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();

        let addr = std::ptr::from_ref(self.inner).addr();
        if state.last_addr != addr {
            state.last_addr = addr;
            *state.cache.borrow_mut() = None;
        }
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        let bpm = self.inner.meter.bpm.load(Acquire);
        let global_start = self
            .inner
            .position
            .get_global_start()
            .in_samples_f(bpm, self.inner.meter.sample_rate);
        let global_end = self
            .inner
            .position
            .get_global_end()
            .in_samples_f(bpm, self.inner.meter.sample_rate);
        let pixel_size = self.scale.x.exp2();

        Node::new(Size::new(
            (global_end - global_start) / pixel_size,
            self.scale.y,
        ))
        .translate(Vector::new(
            (global_start - self.position.x) / pixel_size,
            0.0,
        ))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if let Event::Window(window::Event::RedrawRequested(..)) = event {
            if state.last_bounds != bounds {
                state.last_bounds = bounds;
                *state.cache.borrow_mut() = None;
            }

            if state.last_viewport != *viewport {
                state.last_viewport = *viewport;
                *state.cache.borrow_mut() = None;
            }

            return;
        }

        if shell.is_event_captured() {
            return;
        }

        if let Event::Mouse(mouse::Event::CursorMoved { .. }) = event {
            let interaction = Self::interaction(bounds, cursor, viewport);

            if interaction != state.interaction {
                state.interaction = interaction;
                shell.request_redraw();
            }
        }
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

        // the bounds of the clip header
        let mut upper_bounds = bounds;
        upper_bounds.height = upper_bounds.height.min(LINE_HEIGHT);

        let color = if self.enabled {
            theme.extended_palette().primary.weak.color
        } else {
            theme.extended_palette().secondary.weak.color
        };

        // the opaque background of the clip header
        let text_background = Quad {
            bounds: upper_bounds,
            ..Quad::default()
        };
        renderer.fill_quad(text_background, color);

        let state = tree.state.downcast_ref::<State>();

        // the text containing the name of the sample
        let text = Text {
            content: self.inner.audio.name.as_ref().into(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: renderer.default_size(),
            line_height: LineHeight::default(),
            font: renderer.default_font(),
            align_x: Alignment::Left,
            align_y: Vertical::Top,
            shaping: state.shaping,
            wrapping: Wrapping::None,
        };
        renderer.fill_text(
            text,
            upper_bounds.position() + Vector::new(3.0, 0.0),
            theme.extended_palette().background.strong.text,
            upper_bounds,
        );

        if bounds.height == upper_bounds.height {
            return;
        }

        // the bounds of the clip body
        let lower_bounds = bounds.shrink(padding::top(upper_bounds.height));

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: lower_bounds,
            ..Quad::default()
        };
        renderer.fill_quad(clip_background, color.scale_alpha(0.2));

        // clear the mesh cache if the theme has changed
        if state
            .last_theme
            .borrow()
            .as_ref()
            .is_none_or(|last_theme| *last_theme != *theme)
        {
            *state.last_theme.borrow_mut() = Some(theme.clone());
            *state.cache.borrow_mut() = None;
        }

        // fill the mesh cache if it's cleared
        if state.cache.borrow().is_none() {
            if let Some(mesh) = waveform::mesh(
                &self.inner.meter,
                self.inner.position.get_global_start(),
                self.inner.position.get_clip_start(),
                &self.inner.audio.lods,
                self.position,
                self.scale,
                theme,
                Point::new(bounds.x, layout.position().y),
                lower_bounds,
            ) {
                state.cache.borrow_mut().replace(
                    Geometry::Live {
                        meshes: vec![mesh],
                        images: Vec::new(),
                        text: Vec::new(),
                    }
                    .cache(Group::unique(), None),
                );
            }
        }

        if let Some(cache) = state.cache.borrow().as_ref() {
            // draw the mesh
            renderer.draw_geometry(Geometry::load(cache));
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> Interaction {
        tree.state.downcast_ref::<State>().interaction
    }
}

impl<'a> AudioClip<'a> {
    pub fn new(
        inner: &'a AudioClipInner,
        position: &'a Vec2,
        scale: &'a Vec2,
        enabled: bool,
    ) -> Self {
        Self {
            inner,
            position,
            scale,
            enabled,
        }
    }

    fn interaction(bounds: Rectangle, cursor: Cursor, viewport: &Rectangle) -> Interaction {
        let Some(mut cursor) = cursor.position() else {
            return Interaction::default();
        };

        if !bounds
            .intersection(viewport)
            .is_some_and(|bounds| bounds.contains(cursor))
        {
            return Interaction::default();
        }

        cursor.x -= bounds.x;

        if cursor.x < 10.0 || bounds.width - cursor.x < 10.0 {
            Interaction::ResizingHorizontally
        } else {
            Interaction::Grab
        }
    }
}

impl<'a, Message> From<AudioClip<'a>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: AudioClip<'a>) -> Self {
        Self::new(value)
    }
}
