use super::arrangement::SWM;
use iced::{
    Border, Element, Event, Length, Padding, Point, Rectangle, Renderer, Size, Theme,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{self, Limits, Node},
        renderer::{Quad, Style},
        widget::{Tree, tree},
    },
    event::Status,
    mouse::{self, Cursor, ScrollDelta},
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
    inner: Element<'a, Message, Theme, Renderer>,
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

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> Status {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed(mouse::Button::Left)
                    if state.dragging.is_none() && cursor.is_over(bounds) =>
                {
                    let pos = cursor.position().unwrap();
                    state.dragging = Some(pos.y);
                    return Status::Captured;
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) if state.dragging.is_some() => {
                    state.dragging = None;
                    return Status::Captured;
                }
                mouse::Event::CursorMoved {
                    position: Point { y, .. },
                } => {
                    if let Some(last) = state.dragging {
                        let diff = (y - last) * 0.1;

                        state.current = state
                            .current
                            .saturating_add_signed(-diff as i16)
                            .clamp(*self.range.start(), *self.range.end());
                        state.dragging = Some(diff.fract().mul_add(10.0, last));

                        shell.publish((self.f)(state.current));

                        return Status::Captured;
                    }
                }
                mouse::Event::WheelScrolled { delta }
                    if state.dragging.is_none() && cursor.is_over(bounds) =>
                {
                    let diff = match delta {
                        ScrollDelta::Lines { y, .. } => y,
                        ScrollDelta::Pixels { y, .. } => y / SWM,
                    } + state.scroll;

                    state.current = state
                        .current
                        .saturating_add_signed(diff as i16)
                        .clamp(*self.range.start(), *self.range.end());
                    state.scroll = diff.fract();

                    shell.publish((self.f)(state.current));

                    return Status::Captured;
                }
                _ => {}
            }
        }

        Status::Ignored
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
                .color(theme.extended_palette().secondary.base.text),
            ..Quad::default()
        };

        renderer.fill_quad(background, theme.extended_palette().secondary.weak.color);

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

impl<'a, Message> From<BpmInput<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
{
    fn from(knob: BpmInput<'a, Message>) -> Self {
        Self::new(knob)
    }
}
