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

        let y_scale = self.scale.y.load(SeqCst);
        let y_offset = self.position.y.load(SeqCst);

        self.tracks
            .read()
            .unwrap()
            .iter()
            .enumerate()
            .for_each(|(i, track)| {
                let track_bounds = Rectangle::new(
                    Point::new(
                        bounds.x,
                        y_offset.mul_add(-y_scale, (i as f32).mul_add(y_scale, bounds.y)),
                    ),
                    Size::new(bounds.width, y_scale),
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

        let numerator = self.meter.numerator.load(SeqCst);
        let x_position = self.position.x.load(SeqCst);
        let x_scale = self.scale.x.load(SeqCst).exp2();

        let mut beat = Position::from_interleaved_samples(x_position as u32, &self.meter);
        if beat.sub_quarter_note != 0 {
            beat.sub_quarter_note = 0;
            beat.quarter_note += 1;
        }

        let mut end_beat =
            beat + Position::from_interleaved_samples((bounds.width * x_scale) as u32, &self.meter);
        end_beat.sub_quarter_note = 0;

        while beat <= end_beat {
            let color = if x_scale > 11f32.exp2() {
                if beat.quarter_note % u16::from(numerator) == 0 {
                    let bar = beat.quarter_note / u16::from(numerator);
                    if bar % 4 == 0 {
                        theme.extended_palette().secondary.strong.color
                    } else {
                        theme.extended_palette().secondary.weak.color
                    }
                } else {
                    beat.quarter_note += 1;
                    continue;
                }
            } else if beat.quarter_note % u16::from(numerator) == 0 {
                theme.extended_palette().secondary.strong.color
            } else {
                theme.extended_palette().secondary.weak.color
            };

            let path = Path::new(|path| {
                let x = (beat.in_interleaved_samples(&self.meter) as f32 - x_position) / x_scale;
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
            let x = (self.meter.global_time.load(SeqCst) as f32 - self.position.x.load(SeqCst))
                / self.scale.x.load(SeqCst).exp2();
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
