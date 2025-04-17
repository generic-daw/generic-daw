use super::{LINE_HEIGHT, Vec2, waveform};
use generic_daw_core::{Meter, Position, Recording as RecordingInner};
use iced::{
    Element, Fill, Length, Point, Rectangle, Renderer, Shrink, Size, Theme, Vector,
    advanced::{
        Layout, Renderer as _, Text, Widget,
        graphics::mesh::Renderer as _,
        layout::{Limits, Node},
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::Tree,
    },
    alignment::Vertical,
    mouse::Cursor,
    padding,
    widget::text::{Alignment, LineHeight, Shaping, Wrapping},
};
use std::{cmp::min_by, sync::atomic::Ordering::Acquire};

#[derive(Clone, Debug)]
pub struct Recording<'a> {
    inner: &'a RecordingInner,
    meter: &'a Meter,
    position: &'a Vec2,
    scale: &'a Vec2,
}

impl<Message> Widget<Message, Theme, Renderer> for Recording<'_> {
    fn size(&self) -> Size<Length> {
        Size::new(Shrink, Fill)
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        let bpm = self.meter.bpm.load(Acquire);
        let global_start = self
            .inner
            .position
            .in_samples_f(bpm, self.meter.sample_rate);
        let len = self.inner.len() as f32;
        let pixel_size = self.scale.x.exp2();

        Node::new(Size::new(len / pixel_size, self.scale.y)).translate(Vector::new(
            (global_start - self.position.x) / pixel_size,
            0.0,
        ))
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
        upper_bounds.height = min_by(upper_bounds.height, LINE_HEIGHT, f32::total_cmp);

        let color = theme.extended_palette().danger.weak.color;

        // the opaque background of the clip header
        let text_background = Quad {
            bounds: upper_bounds,
            ..Quad::default()
        };
        renderer.fill_quad(text_background, color);

        // the text containing the name of the sample
        let text = Text {
            content: self.inner.name.as_ref().into(),
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
            upper_bounds.position() + Vector::new(3.0, 0.0),
            theme.extended_palette().background.strong.text,
            upper_bounds,
        );

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
        renderer.fill_quad(clip_background, color.scale_alpha(0.25));

        // we don't need to cache here, since `RecordingInner` only exists as long
        // as we are recording
        if let Some(waveform) = waveform::mesh(
            self.meter,
            self.inner.position,
            Position::ZERO,
            &self.inner.lods,
            self.position,
            self.scale,
            theme,
            Point::new(bounds.x, layout.position().y),
            lower_bounds,
        ) {
            // draw the mesh
            renderer.draw_mesh(waveform);
        }
    }
}

impl<'a> Recording<'a> {
    pub fn new(
        inner: &'a RecordingInner,
        meter: &'a Meter,
        position: &'a Vec2,
        scale: &'a Vec2,
    ) -> Self {
        Self {
            inner,
            meter,
            position,
            scale,
        }
    }
}

impl<'a, Message> From<Recording<'a>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: Recording<'a>) -> Self {
        Self::new(value)
    }
}
