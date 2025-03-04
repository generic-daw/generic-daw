use super::LINE_HEIGHT;
use iced::{
    Color, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        renderer::{Quad, Style},
        widget::{Tree, tree},
    },
    mouse::Cursor,
    window,
};
use std::{
    cmp::{max_by, min_by},
    fmt::{Debug, Formatter},
    time::Instant,
};

const WIDTH: f32 = LINE_HEIGHT / 3.0 * 4.0 + 2.0;

struct State {
    left: f32,
    right: f32,
    left_mix: f32,
    right_mix: f32,
    last_draw: Instant,
}

impl Default for State {
    fn default() -> Self {
        Self {
            left: f32::default(),
            right: f32::default(),
            left_mix: f32::default(),
            right_mix: f32::default(),
            last_draw: Instant::now(),
        }
    }
}

pub struct PeakMeter {
    update: Box<dyn Fn() -> [f32; 2]>,
    enabled: bool,
}

impl Debug for PeakMeter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeakMeter")
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

impl<Message> Widget<Message, Theme, Renderer> for PeakMeter {
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

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let &Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<State>();
            let bounds = layout.bounds();

            let diff = ((now - state.last_draw).as_millis() * 4) as f32;
            state.last_draw = now;

            let [left, right] = (self.update)();

            state.left = if left >= state.left {
                left
            } else {
                state.left.mul_add(diff - 1.0, left) / diff
            };
            state.left_mix = if !self.enabled {
                0.0
            } else if state.left > 1.0 {
                1.0
            } else {
                max_by(0.0, state.left_mix - (diff / 512.0), f32::total_cmp)
            };

            state.right = if right >= state.right {
                right
            } else {
                state.right.mul_add(diff - 1.0, right) / diff
            };
            state.right_mix = if !self.enabled {
                0.0
            } else if state.right > 1.0 {
                1.0
            } else {
                max_by(0.0, state.right_mix - (diff / 512.0), f32::total_cmp)
            };

            if max_by(state.left, state.right, f32::total_cmp) * bounds.height > 1.0 {
                shell.request_redraw();
            } else {
                state.left = 0.0;
                state.right = 0.0;
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
        let bounds = layout.bounds();

        if !bounds.intersects(viewport) {
            return;
        }

        let state = tree.state.downcast_ref::<State>();

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

impl PeakMeter {
    pub fn new(update: impl Fn() -> [f32; 2] + 'static, enabled: bool) -> Self {
        Self {
            update: Box::new(update),
            enabled,
        }
    }

    fn draw_bar(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        s: f32,
        factor: f32,
        bounds: Rectangle,
    ) {
        let base_color = if self.enabled {
            theme.extended_palette().primary.weak.color
        } else {
            theme.extended_palette().secondary.weak.color
        };

        let foreground_color = mix(
            base_color,
            theme.extended_palette().danger.weak.color,
            factor,
        );

        let background_color = mix(
            base_color,
            theme.extended_palette().background.weak.color,
            0.5,
        );

        let height = bounds.height * min_by(1.0, s, f32::total_cmp);

        let bg = Quad {
            bounds: Rectangle::new(
                bounds.position(),
                Size::new(bounds.width, bounds.height - height),
            ),
            ..Quad::default()
        };
        renderer.fill_quad(bg, background_color);

        let fg = Quad {
            bounds: Rectangle::new(
                bounds.position() + Vector::new(0.0, bounds.height - height),
                Size::new(bounds.width, height),
            ),
            ..Quad::default()
        };
        renderer.fill_quad(fg, foreground_color);
    }
}

impl<Message> From<PeakMeter> for Element<'_, Message> {
    fn from(knob: PeakMeter) -> Self {
        Self::new(knob)
    }
}

fn mix(a: Color, b: Color, factor: f32) -> Color {
    let b_amount = factor.clamp(0.0, 1.0);
    let a_amount = 1.0 - b_amount;

    let a_linear = a.into_linear().map(|c| c * a_amount);
    let b_linear = b.into_linear().map(|c| c * b_amount);

    Color::from_linear_rgba(
        a_linear[0] + b_linear[0],
        a_linear[1] + b_linear[1],
        a_linear[2] + b_linear[2],
        a_linear[3] + b_linear[3],
    )
}
