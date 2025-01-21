use super::{ArrangementPosition, ArrangementScale, MeshExt as _, TrackClip};
use generic_daw_core::{Meter, Track as TrackInner, TrackClip as TrackClipInner};
use iced::{
    advanced::{
        graphics::Mesh,
        layout::{Limits, Node},
        renderer::Style,
        widget::Tree,
        Layout, Renderer as _, Widget,
    },
    mouse::{Cursor, Interaction},
    Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Clone)]
pub struct Track<'a, Message> {
    inner: Arc<TrackInner>,
    /// the position of the top left corner of the arrangement viewport
    position: Rc<ArrangementPosition>,
    /// information about the scale of the timeline viewport
    scale: Rc<ArrangementScale>,
    /// list of all the clip widgets
    clips: Rc<RefCell<Vec<Element<'a, Message, Theme, Renderer>>>>,
}

impl<Message> Widget<Message, Theme, Renderer> for Track<'_, Message> {
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
        self.clips.borrow_mut().clear();
        self.clips.borrow_mut().extend(
            self.inner
                .clips()
                .read()
                .unwrap()
                .iter()
                .map(|clip| TrackClip::new(clip.clone(), self.scale.clone()))
                .map(Element::new),
        );

        self.diff(tree);

        let meter = self.inner.meter();

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
                        (clip.get_global_start().in_interleaved_samples_f(meter)
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
                renderer.with_layer(bounds, |renderer| {
                    child
                        .as_widget()
                        .draw(tree, renderer, theme, style, layout, cursor, &bounds);
                });
            });
    }
}

impl<Message> Track<'_, Message> {
    pub fn new(
        inner: Arc<TrackInner>,
        position: Rc<ArrangementPosition>,
        scale: Rc<ArrangementScale>,
    ) -> Self {
        Self {
            inner,
            position,
            scale,
            clips: Rc::default(),
        }
    }

    pub fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        viewport: Rectangle,
        position: &ArrangementPosition,
        scale: &ArrangementScale,
    ) -> Vec<Mesh> {
        let meter = self.inner.meter();

        self.inner
            .clips()
            .read()
            .unwrap()
            .iter()
            .filter_map(|clip| {
                let first_pixel = (clip.get_global_start().in_interleaved_samples_f(meter)
                    - position.x.get())
                    / scale.x.get().exp2()
                    + bounds.x;

                let last_pixel = (clip.get_global_end().in_interleaved_samples_f(meter)
                    - position.x.get())
                    / scale.x.get().exp2()
                    + bounds.x;

                Rectangle::new(
                    Point::new(first_pixel, bounds.y),
                    Size::new(last_pixel - first_pixel, bounds.height),
                )
                .intersection(&bounds)
                .and_then(|bounds| clip.meshes(theme, bounds, viewport, position, scale))
            })
            .collect()
    }

    pub fn get_clip_at_global_time(
        &self,
        meter: &Arc<Meter>,
        global_time: usize,
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
