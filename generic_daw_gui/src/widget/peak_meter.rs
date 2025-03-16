use super::LINE_HEIGHT;
use generic_daw_utils::NoDebug;
use iced::{
    Animation, Color, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        renderer::{Quad, Style},
        widget::{Tree, tree},
    },
    animation::Easing,
    mouse::Cursor,
    window,
};
use std::{
    cmp::min_by,
    time::{Duration, Instant},
};

const WIDTH: f32 = LINE_HEIGHT / 3.0 * 4.0 + 2.0;

#[derive(Debug)]
struct State {
    left: Animation<f32>,
    right: Animation<f32>,
    left_mix: Animation<f32>,
    right_mix: Animation<f32>,
    now: Instant,
}

impl Default for State {
    fn default() -> Self {
        Self {
            left: Animation::new(0.0),
            right: Animation::new(0.0),
            left_mix: Animation::new(0.0),
            right_mix: Animation::new(0.0),
            now: Instant::now(),
        }
    }
}

#[derive(Debug)]
pub struct PeakMeter<F>
where
    F: Fn() -> [f32; 2],
{
    update: NoDebug<F>,
    enabled: bool,
}

impl<F, Message> Widget<Message, Theme, Renderer> for PeakMeter<F>
where
    F: Fn() -> [f32; 2],
{
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
        _layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let &Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<State>();
            state.now = now;

            let [left, right] = (self.update)();

            if left >= state.left.interpolate_with(|v| v, now) {
                state.left = Animation::new(left)
                    .duration(Duration::from_secs_f32(left.exp2()))
                    .easing(Easing::EaseOutExpo)
                    .go(0.0);
            }

            if right >= state.right.interpolate_with(|v| v, now) {
                state.right = Animation::new(right)
                    .duration(Duration::from_secs_f32(right.exp2()))
                    .easing(Easing::EaseOutExpo)
                    .go(0.0);
            }

            if self.enabled {
                if state.left.interpolate_with(|v| v, now) > 1.0 {
                    state.left_mix = Animation::new(1.0).very_quick().go(0.0);
                }

                if state.right.interpolate_with(|v| v, now) > 1.0 {
                    state.right_mix = Animation::new(1.0).very_quick().go(0.0);
                }
            } else {
                state.left_mix = Animation::new(0.0);
                state.right_mix = Animation::new(0.0);
            }

            if state.left.is_animating(now)
                || state.right.is_animating(now)
                || state.left_mix.is_animating(now)
                || state.right_mix.is_animating(now)
            {
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
        let bounds = layout.bounds();

        if !bounds.intersects(viewport) {
            return;
        }

        let state = tree.state.downcast_ref::<State>();

        self.draw_bar(
            renderer,
            theme,
            state.left.interpolate_with(|v| v, state.now),
            state.left_mix.interpolate_with(|v| v, state.now),
            Rectangle::new(
                bounds.position(),
                Size::new(bounds.width / 2.0 - 1.0, bounds.height),
            ),
        );

        self.draw_bar(
            renderer,
            theme,
            state.right.interpolate_with(|v| v, state.now),
            state.right_mix.interpolate_with(|v| v, state.now),
            Rectangle::new(
                bounds.position() + Vector::new(bounds.width / 2.0 + 1.0, 0.0),
                Size::new(bounds.width / 2.0 - 1.0, bounds.height),
            ),
        );
    }
}

impl<F> PeakMeter<F>
where
    F: Fn() -> [f32; 2],
{
    pub fn new(update: F, enabled: bool) -> Self {
        Self {
            update: update.into(),
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

impl<F, Message> From<PeakMeter<F>> for Element<'_, Message>
where
    F: Fn() -> [f32; 2] + 'static,
{
    fn from(value: PeakMeter<F>) -> Self {
        Self::new(value)
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
