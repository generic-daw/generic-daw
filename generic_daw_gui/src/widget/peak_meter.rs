use super::LINE_HEIGHT;
use color_ext::ColorExt as _;
use iced::{
    advanced::{
        layout::{Limits, Node},
        renderer::{Quad, Style},
        widget::{tree, Tree},
        Clipboard, Layout, Renderer as _, Shell, Widget,
    },
    event::Status,
    mouse::Cursor,
    window, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
};
use std::cmp::{max_by, min_by};

mod color_ext;

const DECAY: f32 = 64.0;
const WIDTH: f32 = LINE_HEIGHT / 3.0 * 4.0 + 2.0;

#[derive(Default)]
struct State {
    left: f32,
    right: f32,
    left_mix: f32,
    right_mix: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PeakMeter<Message> {
    left: f32,
    right: f32,
    enabled: bool,
    animate: fn() -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for PeakMeter<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(WIDTH), Length::Fill)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        Node::new(Size::new(WIDTH, limits.max().height))
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> Status {
        if let Event::Window(window::Event::RedrawRequested(..)) = event {
            let state = tree.state.downcast_mut::<State>();
            let bounds = layout.bounds();

            state.left = if self.left >= state.left {
                self.left
            } else {
                state.left.mul_add(DECAY - 1.0, self.left) / DECAY
            };
            self.left = 0.0;
            state.left_mix = if state.left > 1.0 {
                1.0
            } else {
                max_by(0.0, state.left_mix - 0.1, f32::total_cmp)
            };

            state.right = if self.right >= state.right {
                self.right
            } else {
                state.right.mul_add(DECAY - 1.0, self.right) / DECAY
            };
            self.right = 0.0;
            state.right_mix = if state.right > 1.0 {
                1.0
            } else {
                max_by(0.0, state.right_mix - 0.1, f32::total_cmp)
            };

            if max_by(state.left, state.right, f32::total_cmp) * bounds.height > 1.0 {
                shell.publish((self.animate)());
            } else {
                state.left = 0.0;
                state.right = 0.0;
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
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();

        self.draw_bar(
            renderer,
            theme,
            state.left,
            state.left_mix,
            Rectangle::new(
                bounds.position(),
                Size::new(bounds.width / 2.0 - 1.0, bounds.height),
            ),
        );

        self.draw_bar(
            renderer,
            theme,
            state.right,
            state.right_mix,
            Rectangle::new(
                bounds.position() + Vector::new(bounds.width / 2.0 + 1.0, 0.0),
                Size::new(bounds.width / 2.0 - 1.0, bounds.height),
            ),
        );
    }
}

impl<Message> PeakMeter<Message> {
    pub fn new(left: f32, right: f32, enabled: bool, animate: fn() -> Message) -> Self {
        Self {
            left,
            right,
            enabled,
            animate,
        }
    }

    fn draw_bar(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        s: f32,
        mix: f32,
        bounds: Rectangle,
    ) {
        let color = if self.enabled {
            theme
                .extended_palette()
                .primary
                .base
                .color
                .mix(theme.extended_palette().danger.base.color, mix)
        } else {
            theme.extended_palette().secondary.strong.color
        };

        let height = bounds.height * min_by(1.0, s, f32::total_cmp);

        let bg = Quad {
            bounds: Rectangle::new(
                bounds.position(),
                Size::new(bounds.width, bounds.height - height),
            ),
            ..Quad::default()
        };
        renderer.fill_quad(bg, color.scale_alpha(0.5));

        let fg = Quad {
            bounds: Rectangle::new(
                bounds.position() + Vector::new(0.0, bounds.height - height),
                Size::new(bounds.width, height),
            ),
            ..Quad::default()
        };
        renderer.fill_quad(fg, color);
    }
}

impl<'a, Message> From<PeakMeter<Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
{
    fn from(knob: PeakMeter<Message>) -> Self {
        Self::new(knob)
    }
}
