use super::LINE_HEIGHT;
use iced::{
    Animation, Border, Color, Element, Event, Length, Rectangle, Renderer, Size, Theme,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        renderer::{Quad, Style},
        widget::{Tree, tree},
    },
    mouse::Cursor,
    window,
};
use std::{convert::identity, time::Instant};

struct State {
    last_enabled: bool,
    animation: Animation<f32>,
    now: Instant,
}

impl State {
    fn new(enabled: bool) -> Self {
        Self {
            last_enabled: enabled,
            animation: Animation::new(f32::from(u8::from(enabled))),
            now: Instant::now(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AnimatedDot {
    enabled: bool,
    radius: f32,
}

impl<Message> Widget<Message, Theme, Renderer> for AnimatedDot {
    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(2.0 * self.radius),
            Length::Fixed(2.0 * self.radius),
        )
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.enabled))
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        Node::new(Size::new(2.0 * self.radius, 2.0 * self.radius))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<State>();
            state.now = *now;

            if self.enabled != state.last_enabled {
                state.animation =
                    Animation::new(state.animation.interpolate_with(identity, state.now))
                        .very_quick()
                        .go(f32::from(u8::from(self.enabled)));
                state.last_enabled = self.enabled;
            }
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        style: &Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let mut bounds = layout.bounds();
        let border = Border::default()
            .rounded(f32::INFINITY)
            .color(style.text_color);

        let outline = Quad {
            bounds,
            border: border.width(2.0),
            ..Quad::default()
        };

        renderer.fill_quad(outline, Color::TRANSPARENT);

        let state = tree.state.downcast_ref::<State>();

        let factor = state.animation.interpolate_with(identity, state.now);
        if factor == 0.0 {
            return;
        }

        let offset = self.radius * (1.0 - factor);

        bounds.x += offset;
        bounds.y += offset;
        bounds.width *= factor;
        bounds.height *= factor;

        let inner = Quad {
            bounds,
            border,
            ..Quad::default()
        };

        renderer.fill_quad(inner, style.text_color);
    }
}

impl AnimatedDot {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            radius: LINE_HEIGHT,
        }
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }
}

impl<Message> From<AnimatedDot> for Element<'_, Message> {
    fn from(value: AnimatedDot) -> Self {
        Element::new(value)
    }
}
