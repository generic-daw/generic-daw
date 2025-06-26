use super::{LINE_HEIGHT, Vec2};
use generic_daw_core::{self as core, RtState};
use iced::{
    Element, Event, Fill, Length, Point, Rectangle, Renderer, Shrink, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse::{Click, click::Kind},
        renderer::{Quad, Style},
        widget::{Tree, tree},
    },
    mouse::{self, Cursor, Interaction},
    padding,
};

#[derive(Default)]
struct State {
    last_click: Option<Click>,
}

#[derive(Clone, Debug)]
pub struct MidiClip<'a, Message> {
    inner: &'a core::MidiClip,
    rtstate: &'a RtState,
    position: &'a Vec2,
    scale: &'a Vec2,
    enabled: bool,
    on_double_click: Message,
}

impl<Message> Widget<Message, Theme, Renderer> for MidiClip<'_, Message>
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
        Size::new(Shrink, Fill)
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        let start = self.inner.position.start().to_samples_f(self.rtstate);
        let end = self.inner.position.end().to_samples_f(self.rtstate);
        let pixel_size = self.scale.x.exp2();

        Node::new(Size::new((end - start) / pixel_size, self.scale.y))
            .translate(Vector::new((start - self.position.x) / pixel_size, 0.0))
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
        _viewport: &Rectangle,
    ) {
        if shell.is_event_captured() {
            return;
        }

        if let Event::Mouse(mouse::Event::ButtonPressed {
            button: mouse::Button::Left,
            ..
        }) = event
            && let Some(cursor) = cursor.position_in(layout.bounds())
        {
            let state = tree.state.downcast_mut::<State>();

            let new_click = Click::new(cursor, mouse::Button::Left, state.last_click);
            state.last_click = Some(new_click);

            if new_click.kind() == Kind::Double {
                shell.publish(self.on_double_click.clone());
                shell.capture_event();
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

        let pattern = self.inner.pattern.load();

        let (min, max) = pattern.iter().fold((255, 0), |(min, max), note| {
            (note.key.0.min(min), note.key.0.max(max))
        });

        // there is nothing to draw
        if min > max {
            return;
        }

        // how tall each note is, giving each note equal space
        // adding one space above the top note and below the bottom note
        let note_height = (self.scale.y - LINE_HEIGHT) / f32::from(max - min + 3);

        let offset = self.inner.position.offset();
        let pixel_size = self.scale.x.exp2();

        let position = Vector::new(lower_bounds.x, layout.position().y);
        let clip_bounds = Rectangle::new(
            Point::new(0.0, lower_bounds.y - position.y),
            lower_bounds.size(),
        );

        for note in &**pattern {
            let start_pixel = (note.start.saturating_sub(offset).to_samples_f(self.rtstate)
                - self.position.x)
                / pixel_size;
            let end_pixel = (note.end.saturating_sub(offset).to_samples_f(self.rtstate)
                - self.position.x)
                / pixel_size;

            let top_pixel = f32::from(max - note.key.0 + 1).mul_add(note_height, LINE_HEIGHT);

            let note_bounds = Rectangle::new(
                Point::new(start_pixel, top_pixel),
                Size::new(end_pixel - start_pixel, note_height),
            );

            let Some(note_bounds) = note_bounds.intersection(&clip_bounds) else {
                continue;
            };

            let note = Quad {
                bounds: note_bounds + position,
                ..Quad::default()
            };

            renderer.fill_quad(note, theme.extended_palette().background.strong.text);
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> Interaction {
        let Some(bounds) = layout.bounds().intersection(viewport) else {
            return Interaction::default();
        };

        let Some(cursor) = cursor.position_in(bounds) else {
            return Interaction::default();
        };

        if cursor.x < 10.0 || bounds.width - cursor.x < 10.0 {
            Interaction::ResizingHorizontally
        } else {
            Interaction::Grab
        }
    }
}

impl<'a, Message> MidiClip<'a, Message> {
    pub fn new(
        inner: &'a core::MidiClip,
        rtstate: &'a RtState,
        position: &'a Vec2,
        scale: &'a Vec2,
        enabled: bool,
        on_double_click: Message,
    ) -> Self {
        Self {
            inner,
            rtstate,
            position,
            scale,
            enabled,
            on_double_click,
        }
    }
}

impl<'a, Message> From<MidiClip<'a, Message>> for Element<'a, Message>
where
    Message: Clone + 'a,
{
    fn from(value: MidiClip<'a, Message>) -> Self {
        Self::new(value)
    }
}
