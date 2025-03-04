use super::arrangement::SWM;
use iced::{
    Border, Element, Event, Length, Padding, Point, Rectangle, Renderer, Size, Theme,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{self, Limits, Node},
        renderer::{Quad, Style},
        widget::{Tree, tree},
    },
    mouse::{self, Cursor, Interaction, ScrollDelta},
    widget::Text,
};
use std::{
    fmt::{Debug, Formatter},
    ops::RangeInclusive,
};

struct State {
    dragging: Option<f32>,
    scroll: f32,
    current: u16,
}

impl State {
    pub fn new(current: u16) -> Self {
        Self {
            dragging: None,
            scroll: 0.0,
            current,
        }
    }
}

pub struct BpmInput<'a, Message> {
    inner: Element<'a, Message>,
    range: RangeInclusive<u16>,
    current: u16,
    f: fn(u16) -> Message,
}

impl<Message> Debug for BpmInput<'_, Message> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BpmInput")
            .field("current", &self.current)
            .finish_non_exhaustive()
    }
}

impl<Message> Widget<Message, Theme, Renderer> for BpmInput<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.inner)]
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.current))
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        layout::padded(
            limits,
            Length::Shrink,
            Length::Shrink,
            Padding::new(5.0),
            |limits| {
                self.inner
                    .as_widget()
                    .layout(&mut tree.children[0], renderer, limits)
            },
        )
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

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed {
                    button: mouse::Button::Left,
                    ..
                } if state.dragging.is_none() && cursor.is_over(bounds) => {
                    let pos = cursor.position().unwrap();
                    state.dragging = Some(pos.y.trunc());
                    shell.capture_event();
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) if state.dragging.is_some() => {
                    state.dragging = None;
                    shell.capture_event();
                }
                mouse::Event::CursorMoved {
                    position: Point { y, .. },
                    ..
                } => {
                    if let Some(last) = state.dragging {
                        let diff = ((y - last) * 0.1).trunc();

                        state.current = state
                            .current
                            .saturating_add_signed(-diff as i16)
                            .clamp(*self.range.start(), *self.range.end());
                        state.dragging = Some(diff.mul_add(10.0, last));

                        shell.publish((self.f)(state.current));
                        shell.capture_event();
                    }
                }
                mouse::Event::WheelScrolled { delta, .. }
                    if state.dragging.is_none() && cursor.is_over(bounds) =>
                {
                    let diff = match delta {
                        ScrollDelta::Lines { y, .. } => *y,
                        ScrollDelta::Pixels { y, .. } => y / SWM,
                    } + state.scroll;

                    state.current = state
                        .current
                        .saturating_add_signed(diff as i16)
                        .clamp(*self.range.start(), *self.range.end());
                    state.scroll = diff.fract();

                    shell.publish((self.f)(state.current));
                    shell.capture_event();
                }
                _ => {}
            }
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        let background = Quad {
            bounds,
            border: Border::default()
                .width(1.0)
                .color(theme.extended_palette().background.strong.color),
            ..Quad::default()
        };

        renderer.fill_quad(background, theme.extended_palette().background.weak.color);

        self.inner.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> Interaction {
        if cursor.is_over(layout.bounds()) || tree.state.downcast_ref::<State>().dragging.is_some()
        {
            Interaction::ResizingVertically
        } else {
            Interaction::default()
        }
    }
}

impl<Message> BpmInput<'_, Message> {
    pub fn new(current: u16, range: RangeInclusive<u16>, f: fn(u16) -> Message) -> Self {
        let inner = Text::new(current).width(29.0).into();

        Self {
            inner,
            range,
            current,
            f,
        }
    }
}

impl<'a, Message> From<BpmInput<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(knob: BpmInput<'a, Message>) -> Self {
        Self::new(knob)
    }
}
