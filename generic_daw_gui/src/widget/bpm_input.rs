use super::{LINE_HEIGHT, SWM};
use iced::{
    Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Text, Widget,
        layout::{Limits, Node},
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::{Tree, tree},
    },
    alignment::Vertical,
    border,
    mouse::{self, Cursor, Interaction, ScrollDelta},
    widget::text::{Alignment, LineHeight, Shaping, Wrapping},
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

pub struct BpmInput<Message> {
    range: RangeInclusive<u16>,
    current: u16,
    f: fn(u16) -> Message,
}

impl<Message> Debug for BpmInput<Message> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BpmInput")
            .field("current", &self.current)
            .finish_non_exhaustive()
    }
}

impl<Message> Widget<Message, Theme, Renderer> for BpmInput<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(39.0), Length::Fixed(LINE_HEIGHT + 10.0))
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.current))
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        Node::new(Size::new(39.0, LINE_HEIGHT + 10.0))
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
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        let background = Quad {
            bounds,
            border: border::width(1.0).color(theme.extended_palette().background.strong.color),
            ..Quad::default()
        };

        renderer.fill_quad(background, theme.extended_palette().background.weak.color);

        let text = Text {
            content: itoa::Buffer::new().format(self.current).to_owned(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: renderer.default_size(),
            line_height: LineHeight::default(),
            font: renderer.default_font(),
            align_x: Alignment::Left,
            align_y: Vertical::Top,
            shaping: Shaping::Basic,
            wrapping: Wrapping::None,
        };

        renderer.fill_text(
            text,
            bounds.position() + Vector::new(5.0, 5.0),
            theme.extended_palette().background.weak.text,
            bounds,
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

impl<Message> BpmInput<Message> {
    pub fn new(current: u16, range: RangeInclusive<u16>, f: fn(u16) -> Message) -> Self {
        Self { range, current, f }
    }
}

impl<'a, Message> From<BpmInput<Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(knob: BpmInput<Message>) -> Self {
        Self::new(knob)
    }
}
