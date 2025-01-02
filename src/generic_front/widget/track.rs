use super::TrackClip;
use crate::{
    generic_back::{Meter, Track as TrackInner, TrackClip as TrackClipInner},
    generic_front::{TimelinePosition, TimelineScale},
};
use iced::{
    advanced::{
        graphics::Mesh,
        layout::{Limits, Node},
        renderer::Style,
        widget::{tree, Tree},
        Layout, Widget,
    },
    mouse::{Cursor, Interaction},
    Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
};
use std::{
    cell::RefCell,
    fmt::{Debug, Formatter},
    rc::Rc,
    sync::Arc,
};

#[derive(Default)]
struct State {
    clips: RefCell<Vec<TrackClip>>,
}

#[derive(Clone)]
pub struct Track<Message> {
    inner: Arc<TrackInner>,
    /// information about the position of the timeline viewport
    position: Rc<TimelinePosition>,
    /// information about the scale of the timeline viewport
    scale: Rc<TimelineScale>,
    /// list of all the clip widgets
    clips: Rc<RefCell<Vec<Element<'static, Message, Theme, Renderer>>>>,
}

impl<Message> Debug for Track<Message> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("").field(&self.inner).finish_non_exhaustive()
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Track<Message>
where
    Message: 'static,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Shrink,
        }
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.clips.borrow());
    }

    fn children(&self) -> Vec<Tree> {
        self.clips.borrow().iter().map(Tree::new).collect()
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let state = tree.state.downcast_ref::<State>();

        self.inner.clips().read().unwrap().iter().for_each(|clip| {
            let contains = state.clips.borrow().iter().any(|w| w.is(clip));
            if !contains {
                state
                    .clips
                    .borrow_mut()
                    .push(TrackClip::new(clip.clone(), self.scale.clone()));
            }
        });

        *self.clips.borrow_mut() = state
            .clips
            .borrow()
            .iter()
            .map(|clip| clip.clone().into())
            .collect();

        self.diff(tree);

        let meter = match &*self.inner {
            TrackInner::Audio(track) => &track.meter,
            TrackInner::Midi(track) => &track.meter,
        };

        Node::with_children(
            Size::new(limits.max().width, self.scale.y.get()),
            self.clips
                .borrow()
                .iter()
                .zip(&mut tree.children)
                .map(|(widget, tree)| {
                    widget.as_widget().layout(
                        tree,
                        renderer,
                        &Limits::new(limits.min(), Size::new(f32::INFINITY, limits.max().height)),
                    )
                })
                .zip(self.inner.clips().read().unwrap().iter())
                .map(|(node, clip)| {
                    node.translate(Vector::new(
                        (clip.get_global_start().in_interleaved_samples(meter) as f32
                            - self.position.x.get())
                            / self.scale.x.get().exp2(),
                        0.0,
                    ))
                })
                .collect(),
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> Interaction {
        self.clips
            .borrow()
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, tree), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let Some(bounds) = viewport.intersection(&layout.bounds()) else {
            return;
        };

        // TODO fix this iced bug
        // https://github.com/iced-rs/iced/issues/2700
        if bounds.height < 1.0 {
            return;
        }

        self.clips
            .borrow()
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .for_each(|((child, tree), layout)| {
                child
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, &bounds);
            });
    }
}

impl<Message> Track<Message> {
    pub fn new(
        inner: Arc<TrackInner>,
        position: Rc<TimelinePosition>,
        scale: Rc<TimelineScale>,
    ) -> Self {
        Self {
            inner,
            position,
            scale,
            clips: Rc::default(),
        }
    }

    pub fn is(&self, other: &Arc<TrackInner>) -> bool {
        Arc::ptr_eq(&self.inner, other)
    }

    pub fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        viewport: Rectangle,
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
                    clip.meshes(theme, clip_bounds, viewport, position, scale)
                })
            })
            .collect()
    }

    pub fn get_clip_at_global_time(
        &self,
        meter: &Arc<Meter>,
        global_time: u32,
    ) -> Option<Arc<TrackClipInner>> {
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

impl<Message> From<Track<Message>> for Element<'_, Message, Theme, Renderer>
where
    Message: 'static,
{
    fn from(track: Track<Message>) -> Self {
        Self::new(track)
    }
}
