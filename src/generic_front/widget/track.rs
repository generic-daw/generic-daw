mod audio_clip;
mod track_clip;

use crate::{
    generic_back::{Meter, TrackClip, TrackInner},
    generic_front::{TimelinePosition, TimelineScale},
};
use iced::{
    advanced::{
        graphics::Mesh,
        layout::{Limits, Node},
        renderer::Style,
        widget::Tree,
        Layout, Widget,
    },
    mouse::Cursor,
    Length, Point, Rectangle, Renderer, Size, Theme,
};
use std::{rc::Rc, sync::Arc};

#[derive(Clone)]
pub struct Track {
    inner: Arc<TrackInner>,
    /// information about the position of the timeline viewport
    position: Rc<TimelinePosition>,
    /// information about the scale of the timeline viewport
    scale: Rc<TimelineScale>,
}

impl<Message> Widget<Message, Theme, Renderer> for Track {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fixed(self.scale.y.get()),
        }
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        Node::new(Size::new(limits.max().width, self.scale.y.get()))
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

        let meter = match &*self.inner {
            TrackInner::Audio(track) => &track.meter,
            TrackInner::Midi(track) => &track.meter,
        };

        self.inner.clips().read().unwrap().iter().for_each(|clip| {
            let first_pixel = (clip.get_global_start().in_interleaved_samples(meter) as f32
                - self.position.x.get())
                / self.scale.x.get().exp2()
                + bounds.x;

            let last_pixel = (clip.get_global_end().in_interleaved_samples(meter) as f32
                - self.position.x.get())
                / self.scale.x.get().exp2()
                + bounds.x;

            let clip_bounds = Rectangle::new(
                Point::new(first_pixel, bounds.y),
                Size::new(last_pixel - first_pixel, bounds.height),
            );
            let clip_bounds = bounds.intersection(&clip_bounds);
            if let Some(clip_bounds) = clip_bounds {
                clip.draw(renderer, theme, clip_bounds);
            }
        });
    }
}

impl Track {
    pub fn new(
        inner: Arc<TrackInner>,
        position: Rc<TimelinePosition>,
        scale: Rc<TimelineScale>,
    ) -> Self {
        Self {
            inner,
            position,
            scale,
        }
    }

    pub fn is(&self, other: &Arc<TrackInner>) -> bool {
        Arc::ptr_eq(&self.inner, other)
    }

    pub fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        arrangement_bounds: Rectangle,
        position: &TimelinePosition,
        scale: &TimelineScale,
    ) -> Vec<Mesh> {
        let meter = match &*self.inner {
            TrackInner::Audio(track) => &track.meter,
            TrackInner::Midi(track) => &track.meter,
        };

        self.inner
            .clips()
            .read()
            .unwrap()
            .iter()
            .filter_map(|clip| {
                let first_pixel = (clip.get_global_start().in_interleaved_samples(meter) as f32
                    - position.x.get())
                    / scale.x.get().exp2()
                    + bounds.x;

                let last_pixel = (clip.get_global_end().in_interleaved_samples(meter) as f32
                    - position.x.get())
                    / scale.x.get().exp2()
                    + bounds.x;

                let clip_bounds = Rectangle::new(
                    Point::new(first_pixel, bounds.y),
                    Size::new(last_pixel - first_pixel, bounds.height),
                );
                let clip_bounds = bounds.intersection(&clip_bounds);
                clip_bounds.and_then(|clip_bounds| {
                    clip.meshes(theme, clip_bounds, arrangement_bounds, position, scale)
                })
            })
            .collect()
    }

    pub fn get_clip_at_global_time(
        &self,
        meter: &Arc<Meter>,
        global_time: u32,
    ) -> Option<Arc<TrackClip>> {
        self.inner
            .clips()
            .read()
            .unwrap()
            .iter()
            .rev()
            .find_map(|clip| {
                if clip.get_global_start().in_interleaved_samples(meter) <= global_time
                    && global_time <= clip.get_global_end().in_interleaved_samples(meter)
                {
                    Some(clip.clone())
                } else {
                    None
                }
            })
    }
}
