use super::{ArrangementPosition, ArrangementScale, LINE_HEIGHT, shaping_of};
use generic_daw_core::AudioClip as AudioClipInner;
use iced::{
    Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Transformation, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Text, Widget,
        graphics::{color, geometry::Renderer as _},
        layout::{Limits, Node},
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::{Tree, tree},
    },
    alignment::Vertical,
    mouse::{self, Cursor, Interaction},
    widget::text::{Alignment, LineHeight, Shaping, Wrapping},
    window,
};
use iced_wgpu::{
    Geometry,
    geometry::Cache,
    graphics::{
        Mesh,
        cache::{Cached as _, Group},
        mesh::{Indexed, SolidVertex2D},
    },
};
use std::{
    cell::RefCell,
    cmp::{max_by, min_by},
    sync::{Arc, atomic::Ordering::Acquire},
};

#[derive(Default)]
struct State {
    cache: RefCell<Option<Cache>>,
    shaping: Shaping,
    interaction: Interaction,
    last_position: ArrangementPosition,
    last_scale: ArrangementScale,
    last_bounds: Rectangle,
    last_addr: usize,
}

impl State {
    fn new(text: &str) -> Self {
        Self {
            shaping: shaping_of(text),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug)]
pub struct AudioClip {
    inner: Arc<AudioClipInner>,
    /// the name of the sample
    name: Box<str>,
    /// the position of the top left corner of the arrangement viewport
    position: ArrangementPosition,
    /// the scale of the arrangement viewport
    scale: ArrangementScale,
    /// whether the clip is in an enabled track
    enabled: bool,
}

impl<Message> Widget<Message, Theme, Renderer> for AudioClip {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(&self.name))
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Fill,
        }
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        let bpm = self.inner.meter.bpm.load(Acquire);
        let global_start = self
            .inner
            .position
            .get_global_start()
            .in_interleaved_samples_f(bpm, self.inner.meter.sample_rate);
        let global_end = self
            .inner
            .position
            .get_global_end()
            .in_interleaved_samples_f(bpm, self.inner.meter.sample_rate);

        Node::new(Size::new(
            (global_end - global_start) / self.scale.x.exp2(),
            self.scale.y,
        ))
        .translate(Vector::new(
            (global_start - self.position.x) / self.scale.x.exp2(),
            0.0,
        ))
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
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        match event {
            Event::Window(window::Event::RedrawRequested(..)) => {
                if state.last_position != self.position {
                    state.last_position = self.position;
                    *state.cache.borrow_mut() = None;
                }

                if state.last_scale != self.scale {
                    state.last_scale = self.scale;
                    *state.cache.borrow_mut() = None;
                }

                if state.last_bounds != bounds {
                    state.last_bounds = bounds;
                    *state.cache.borrow_mut() = None;
                }

                let addr = Arc::as_ptr(&self.inner).addr();
                if state.last_addr != addr {
                    state.last_addr = addr;
                    *state.cache.borrow_mut() = None;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let interaction = Self::mouse_interaction(bounds, cursor, viewport);

                if interaction != state.interaction {
                    state.interaction = interaction;
                    shell.request_redraw();
                }
            }
            _ => {}
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
        let Some(bounds) = layout.bounds().intersection(viewport) else {
            return;
        };

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

        let state = tree.state.downcast_ref::<State>();

        // the text containing the name of the sample
        let text = Text {
            content: String::from(&*self.name),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: renderer.default_size(),
            line_height: LineHeight::default(),
            font: renderer.default_font(),
            align_x: Alignment::Left,
            align_y: Vertical::Top,
            shaping: state.shaping,
            wrapping: Wrapping::None,
        };
        renderer.fill_text(
            text,
            upper_bounds.position() + Vector::new(4.0, 0.0),
            theme.extended_palette().background.strong.text,
            upper_bounds,
        );

        if bounds.height == upper_bounds.height {
            return;
        }

        // the bounds of the waveform part of the clip
        let mut lower_bounds = bounds;
        lower_bounds.height -= upper_bounds.height;
        lower_bounds.y += upper_bounds.height;

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: lower_bounds,
            ..Quad::default()
        };
        renderer.fill_quad(clip_background, color.scale_alpha(0.25));

        // fill the mesh cache if it's cleared
        if state.cache.borrow().is_none() {
            if let Some(mesh) = self.mesh(theme, bounds.size(), self.position, self.scale) {
                state.cache.borrow_mut().replace(
                    Geometry::Live {
                        meshes: vec![mesh],
                        images: Vec::new(),
                        text: Vec::new(),
                    }
                    .cache(Group::unique(), None),
                );
            }
        }

        if let Some(cache) = state.cache.borrow().as_ref() {
            // draw the mesh
            renderer.with_translation(Vector::new(bounds.x, layout.position().y), |renderer| {
                renderer.draw_geometry(Geometry::load(cache));
            });
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> Interaction {
        Self::mouse_interaction(layout.bounds(), cursor, viewport)
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
            .into();

        Self {
            inner,
            name,
            position,
            scale,
            enabled,
        }
    }

    fn mouse_interaction(bounds: Rectangle, cursor: Cursor, viewport: &Rectangle) -> Interaction {
        let Some(mut cursor) = cursor.position() else {
            return Interaction::default();
        };

        if !viewport.contains(cursor) {
            return Interaction::default();
        }

        cursor.x -= bounds.x;

        if cursor.x < 10.0 || bounds.width - cursor.x < 10.0 {
            Interaction::ResizingHorizontally
        } else {
            Interaction::Grab
        }
    }

    fn mesh(
        &self,
        theme: &Theme,
        size: Size,
        position: ArrangementPosition,
        scale: ArrangementScale,
    ) -> Option<Mesh> {
        // the height of the waveform
        let height = scale.y - LINE_HEIGHT;

        debug_assert!(height >= 0.0);

        // samples of the original audio per sample of lod
        let lod_sample_size = scale.x.floor().exp2();

        // samples of the original audio per pixel
        let pixel_size = scale.x.exp2();

        // samples in the lod per pixel
        let lod_samples_per_pixel = lod_sample_size / pixel_size;

        let color = color::pack(theme.extended_palette().background.strong.text);
        let lod = scale.x as usize - 3;

        let bpm = self.inner.meter.bpm.load(Acquire);

        let diff = max_by(
            0.0,
            position.x
                - self
                    .inner
                    .position
                    .get_global_start()
                    .in_interleaved_samples_f(bpm, self.inner.meter.sample_rate),
            f32::total_cmp,
        );

        let clip_start = self
            .inner
            .position
            .get_clip_start()
            .in_interleaved_samples_f(bpm, self.inner.meter.sample_rate);

        let first_index = ((diff + clip_start) / lod_sample_size) as usize;
        let last_index = first_index + (size.width / lod_samples_per_pixel) as usize;
        let last_index = last_index.min(self.inner.audio.lods[lod].len() - 1);

        // this would result in an empty mesh (and a crash)
        if last_index - first_index < 2 {
            return None;
        }

        // vertices of the waveform
        let vertices = self.inner.audio.lods[lod][first_index..last_index]
            .iter()
            .enumerate()
            .flat_map(|(x, (min, max))| {
                let x = x as f32 * lod_samples_per_pixel;

                [
                    SolidVertex2D {
                        position: [x, min.mul_add(height, LINE_HEIGHT)],
                        color,
                    },
                    SolidVertex2D {
                        position: [x, max.mul_add(height, LINE_HEIGHT)],
                        color,
                    },
                ]
            })
            .collect::<Vec<_>>();

        // triangles of the waveform
        let indices = (0..vertices.len() as u32 - 2)
            .flat_map(|i| [i, i + 1, i + 2])
            .collect();

        // the waveform mesh
        Some(Mesh::Solid {
            buffers: Indexed { vertices, indices },
            transformation: Transformation::IDENTITY,
            clip_bounds: Rectangle::new(Point::new(0.0, scale.y - size.height + LINE_HEIGHT), size),
        })
    }
}

impl<'a, Message> From<AudioClip> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: AudioClip) -> Self {
        Self::new(value)
    }
}
