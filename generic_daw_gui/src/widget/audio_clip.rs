use super::{ArrangementPosition, ArrangementScale, LINE_HEIGHT};
use generic_daw_core::AudioClip as AudioClipInner;
use iced::{
    Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Text, Widget,
        layout::{Limits, Node},
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::{Tree, tree},
    },
    alignment::{Horizontal, Vertical},
    event::Status,
    mouse::{Cursor, Interaction},
    widget::text::{LineHeight, Shaping, Wrapping},
    window,
};
use iced_wgpu::primitive::Renderer as _;
use primitive::Primitive;
use sample::Sample;
use std::{
    cell::{Cell, RefCell},
    cmp::{max_by, min_by},
    sync::Arc,
};

mod pipeline;
mod primitive;
mod sample;

#[derive(Default)]
struct State {
    /// the position from the last draw
    last_position: ArrangementPosition,
    /// the scale from the last draw
    last_scale: ArrangementScale,
    /// the size from the last draw
    last_size: Size,
    /// the waveform shader
    primitive: RefCell<Primitive>,
    /// whether to rebuild the shader texture
    rebuild: Cell<bool>,
}

#[derive(Clone, Debug)]
pub struct AudioClip {
    inner: Arc<AudioClipInner>,
    /// the name of the sample
    name: String,
    /// the position of the top left corner of the arrangement viewport
    position: ArrangementPosition,
    /// the scale of the timeline viewport
    scale: ArrangementScale,
    /// whether the clip is in an enabled track
    enabled: bool,
}

impl<Message> Widget<Message, Theme, Renderer> for AudioClip {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Fill,
        }
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        let meter = &self.inner.meter;

        Node::new(Size::new(
            (self.inner.get_global_end().in_interleaved_samples(meter)
                - self.inner.get_global_start().in_interleaved_samples(meter)) as f32
                / self.scale.x.exp2(),
            limits.max().height,
        ))
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> Status {
        if let Event::Window(window::Event::RedrawRequested(..)) = event {
            let state = tree.state.downcast_mut::<State>();

            if state.last_position != self.position {
                state.last_position = self.position;
                state.rebuild.set(true);
            }

            if state.last_scale != self.scale {
                state.last_scale = self.scale;
                state.rebuild.set(true);
            }

            let Some(bounds) = layout.bounds().intersection(viewport) else {
                return Status::Ignored;
            };

            if state.last_size != bounds.size() {
                state.last_size = bounds.size();
                state.rebuild.set(true);
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
        viewport: &Rectangle,
    ) {
        let Some(bounds) = layout.bounds().intersection(viewport) else {
            return;
        };

        // https://github.com/iced-rs/iced/issues/2700
        if bounds.height < 1.0 {
            return;
        }

        // the bounds of the text part of the clip
        let mut upper_bounds = bounds;
        upper_bounds.height = min_by(upper_bounds.height, LINE_HEIGHT, f32::total_cmp);

        let color = if self.enabled {
            theme.extended_palette().primary.weak.color
        } else {
            theme.extended_palette().secondary.weak.color
        };

        // the opaque background of the text
        let text_background = Quad {
            bounds: upper_bounds,
            ..Quad::default()
        };
        renderer.fill_quad(text_background, color);

        // the text containing the name of the sample
        let text = Text {
            content: self.name.clone(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: renderer.default_size(),
            line_height: LineHeight::default(),
            font: renderer.default_font(),
            horizontal_alignment: Horizontal::Left,
            vertical_alignment: Vertical::Top,
            shaping: Shaping::default(),
            wrapping: Wrapping::default(),
        };
        renderer.fill_text(
            text,
            upper_bounds.position() + Vector::new(4.0, -1.0),
            theme.extended_palette().secondary.base.text,
            upper_bounds,
        );

        if bounds.height == upper_bounds.height {
            return;
        }

        // the bounds of the waveform part of the clip
        let mut lower_bounds = bounds;
        lower_bounds.height -= upper_bounds.height;
        lower_bounds.y += upper_bounds.height;

        // https://github.com/iced-rs/iced/issues/2700
        if lower_bounds.height < 1.0 {
            return;
        }

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: lower_bounds,
            ..Quad::default()
        };
        renderer.fill_quad(clip_background, color.scale_alpha(0.25));

        let state = tree.state.downcast_ref::<State>();

        if state.rebuild.get() {
            state.rebuild.set(false);

            let texture = self.texture(
                layout.bounds().intersection(viewport).unwrap().size(),
                self.position,
                self.scale,
            );

            state.primitive.borrow_mut().texture = texture;
        }

        renderer.draw_primitive(lower_bounds, state.primitive.borrow().clone());
    }

    fn mouse_interaction(
        &self,
        _state: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> Interaction {
        let bounds = layout.bounds();

        if let Some(cursor) = cursor.position_in(bounds) {
            if cursor.x < 10.0 || bounds.width - cursor.x < 10.0 {
                return Interaction::ResizingHorizontally;
            }

            return Interaction::Grab;
        }

        Interaction::default()
    }
}

impl AudioClip {
    pub fn new(
        inner: Arc<AudioClipInner>,
        position: ArrangementPosition,
        scale: ArrangementScale,
        enabled: bool,
    ) -> Self {
        let name = inner
            .audio
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        Self {
            inner,
            name,
            position,
            scale,
            enabled,
        }
    }

    fn texture(
        &self,
        size: Size,
        position: ArrangementPosition,
        scale: ArrangementScale,
    ) -> Box<[Sample]> {
        // samples of the original audio per sample of lod
        let lod_sample_size = scale.x.floor().exp2();

        // samples of the original audio per pixel
        let pixel_size = scale.x.exp2();

        // samples in the lod per pixel
        let lod_samples_per_pixel = lod_sample_size / pixel_size;

        let lod = scale.x as usize - 3;

        let diff = max_by(
            0.0,
            position.x
                - self
                    .inner
                    .get_global_start()
                    .in_interleaved_samples_f(&self.inner.meter),
            f32::total_cmp,
        );

        let clip_start = self
            .inner
            .get_clip_start()
            .in_interleaved_samples_f(&self.inner.meter);

        let first_index = ((diff + clip_start) / lod_sample_size) as usize;
        let last_index = first_index + (size.width / lod_samples_per_pixel) as usize;

        // vertices of the waveform
        self.inner.audio.lods[lod][first_index..last_index]
            .iter()
            .map(|&(min, max)| Sample(min, max))
            .collect()
    }
}

impl<'a, Message> From<AudioClip> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
{
    fn from(arrangement_front: AudioClip) -> Self {
        Self::new(arrangement_front)
    }
}
