use super::{LINE_HEIGHT, Vec2};
use generic_daw_core::MidiClip as MidiClipInner;
use iced::{
    Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse::{Click, click::Kind},
        renderer::{Quad, Style},
        widget::{Tree, tree},
    },
    mouse::{self, Cursor, Interaction},
};
use std::{
    cmp::min_by,
    sync::{Arc, atomic::Ordering::Acquire},
};

#[derive(Default)]
struct State {
    interaction: Interaction,
    last_click: Option<Click>,
}

#[derive(Clone, Debug)]
pub struct MidiClip<Message> {
    inner: Arc<MidiClipInner>,
    /// the position of the top left corner of the arrangement viewport
    position: Vec2,
    /// the scale of the arrangement viewport
    scale: Vec2,
    /// whether the clip is in an enabled track
    enabled: bool,
    on_double_click: Message,
}

impl<Message> Widget<Message, Theme, Renderer> for MidiClip<Message>
where
    Message: Clone,
{
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

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        let bpm = self.inner.meter.bpm.load(Acquire);
        let global_start = self
            .inner
            .position
            .get_global_start()
            .in_interleaved_samples_f(bpm, self.inner.meter.sample_rate);
        let global_end = self
            .inner
            .position
            .get_global_end()
            .in_interleaved_samples_f(bpm, self.inner.meter.sample_rate);
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
        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::CursorMoved { .. } => {
                    let interaction = Self::interaction(bounds, cursor, viewport);

                    if interaction != state.interaction {
                        state.interaction = interaction;
                        shell.request_redraw();
                    }
                }
                mouse::Event::ButtonPressed {
                    button: mouse::Button::Left,
                    ..
                } => {
                    if let Some(cursor) = cursor.position() {
                        let state = tree.state.downcast_mut::<State>();

                        let new_click = Click::new(cursor, mouse::Button::Left, state.last_click);
                        state.last_click = Some(new_click);

                        if new_click.kind() == Kind::Double {
                            shell.publish(self.on_double_click.clone());
                            shell.capture_event();
                        }
                    }
                }
                _ => {}
            }
        }
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

        // the bounds of the clip header
        let mut upper_bounds = bounds;
        upper_bounds.height = min_by(upper_bounds.height, LINE_HEIGHT, f32::total_cmp);

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

        if bounds.height == upper_bounds.height {
            return;
        }

        // the bounds of the clip body
        let mut lower_bounds = bounds;
        lower_bounds.height -= upper_bounds.height;
        lower_bounds.y += upper_bounds.height;

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: lower_bounds,
            ..Quad::default()
        };
        renderer.fill_quad(clip_background, color.scale_alpha(0.25));
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

impl<Message> MidiClip<Message> {
    pub fn new(
        inner: Arc<MidiClipInner>,
        position: Vec2,
        scale: Vec2,
        enabled: bool,
        on_double_click: Message,
    ) -> Self {
        Self {
            inner,
            position,
            scale,
            enabled,
            on_double_click,
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

impl<'a, Message> From<MidiClip<Message>> for Element<'a, Message>
where
    Message: Clone + 'a,
{
    fn from(value: MidiClip<Message>) -> Self {
        Self::new(value)
    }
}
