use crate::{
    generic_back::{Arrangement, Position},
    generic_front::TimelineMessage,
};
use iced::{
    advanced::{
        graphics::geometry::Renderer as _,
        layout::{self, Layout},
        renderer,
        widget::{self, Widget},
    },
    mouse,
    widget::canvas::{Frame, Geometry, Path, Stroke},
    Length, Point, Rectangle, Renderer, Size, Theme,
};
use std::sync::{atomic::Ordering::SeqCst, Arc};

impl Widget<TimelineMessage, Theme, Renderer> for Arc<Arrangement> {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(
        &self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::new(limits.max().width, limits.max().height))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        renderer.draw_geometry(self.grid(renderer, bounds, theme));

        self.tracks
            .read()
            .unwrap()
            .iter()
            .enumerate()
            .for_each(|(i, track)| {
                let track_bounds = Rectangle::new(
                    Point::new(
                        bounds.x,
                        self.position.y.load(SeqCst).mul_add(
                            -self.scale.y.load(SeqCst),
                            (i as f32).mul_add(self.scale.y.load(SeqCst), bounds.y),
                        ),
                    ),
                    Size::new(bounds.width, self.scale.y.load(SeqCst)),
                );
                if track_bounds.intersects(&bounds) {
                    track.draw(renderer, theme, track_bounds, bounds);
                }
            });

        renderer.draw_geometry(self.playhead(renderer, bounds, theme));
    }
}

impl Arrangement {
    fn grid(&self, renderer: &Renderer, bounds: Rectangle, theme: &Theme) -> Geometry {
        let mut frame = Frame::new(renderer, bounds.size());

        let mut beat =
            Position::from_interleaved_samples(self.position.x.load(SeqCst) as u32, &self.meter);
        let mut end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * self.scale.x.load(SeqCst).exp2()) as u32,
                &self.meter,
            );
        if beat.sub_quarter_note != 0 {
            beat.sub_quarter_note = 0;
            beat.quarter_note += 1;
        }
        end_beat.sub_quarter_note = 0;

        while beat <= end_beat {
            let color = if self.scale.x.load(SeqCst) > 11.0 {
                if beat.quarter_note % u16::from(self.meter.numerator.load(SeqCst)) == 0 {
                    let bar = beat.quarter_note / u16::from(self.meter.numerator.load(SeqCst));
                    if bar % 4 == 0 {
                        theme.extended_palette().secondary.strong.color
                    } else {
                        theme.extended_palette().secondary.weak.color
                    }
                } else {
                    beat.quarter_note += 1;
                    continue;
                }
            } else if beat.quarter_note % u16::from(self.meter.numerator.load(SeqCst)) == 0 {
                theme.extended_palette().secondary.strong.color
            } else {
                theme.extended_palette().secondary.weak.color
            };

            let path = Path::new(|path| {
                let x = (beat.in_interleaved_samples(&self.meter) as f32
                    - self.position.x.load(SeqCst))
                    / self.scale.x.load(SeqCst).exp2();
                path.line_to(Point::new(x, 0.0));
                path.line_to(Point::new(x, bounds.height));
            });

            frame.with_clip(bounds, |frame| {
                frame.stroke(&path, Stroke::default().with_color(color));
            });
            beat.quarter_note += 1;
        }

        frame.into_geometry()
    }

    fn playhead(&self, renderer: &Renderer, bounds: Rectangle, theme: &Theme) -> Geometry {
        let mut frame = Frame::new(renderer, bounds.size());

        let path = Path::new(|path| {
            let x = -(self.position.x.load(SeqCst)) / self.scale.x.load(SeqCst).exp2()
                + self.meter.global_time.load(SeqCst) as f32 / self.scale.x.load(SeqCst).exp2();
            path.line_to(Point::new(x, 0.0));
            path.line_to(Point::new(x, bounds.height));
        });

        frame.with_clip(bounds, |frame| {
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(theme.extended_palette().primary.base.color)
                    .with_width(2.0),
            );
        });

        frame.into_geometry()
    }
}
