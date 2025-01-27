use super::{border, ArrangementPosition, ArrangementScale, TrackClip, TrackClipExt as _};
use generic_daw_core::{Meter, Track as TrackInner, TrackClip as TrackClipInner};
use iced::{
    advanced::{
        graphics::Mesh,
        layout::{Limits, Node},
        renderer::Style,
        widget::Tree,
        Clipboard, Layout, Renderer as _, Shell, Widget,
    },
    event::Status,
    mouse::{Cursor, Interaction},
    Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
};
use std::{iter::once, sync::Arc};

mod track_ext;

pub use track_ext::TrackExt;

pub struct Track<'a, Message> {
    inner: Arc<TrackInner>,
    /// list of all the clip widgets
    clips: Box<[Element<'a, Message, Theme, Renderer>]>,
    /// the track panel
    panel: Element<'a, Message, Theme, Renderer>,
    /// the position of the top left corner of the arrangement viewport
    position: &'a ArrangementPosition,
    /// the scale of the arrangement viewport
    scale: &'a ArrangementScale,
}

impl<Message> Widget<Message, Theme, Renderer> for Track<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Shrink,
        }
    }

    fn children(&self) -> Vec<Tree> {
        self.clips
            .iter()
            .chain(once(&self.panel))
            .map(Tree::new)
            .collect()
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let meter = self.inner.meter();

        let panel_layout =
            self.panel
                .as_widget()
                .layout(tree.children.last_mut().unwrap(), renderer, limits);
        let panel_width = panel_layout.size().width;

        Node::with_children(
            Size::new(limits.max().width, self.scale.y.get()),
            self.clips
                .iter()
                .zip(&mut tree.children[1..])
                .map(|(widget, tree)| {
                    widget.as_widget().layout(
                        tree,
                        renderer,
                        &Limits::new(limits.min(), Size::new(f32::INFINITY, limits.max().height)),
                    )
                })
                .zip(self.inner.clips().iter())
                .map(|(node, clip)| {
                    node.translate(Vector::new(
                        panel_width
                            + (clip.get_global_start().in_interleaved_samples_f(meter)
                                - self.position.x.get())
                                / self.scale.x.get().exp2(),
                        0.0,
                    ))
                })
                .chain(once(panel_layout))
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
            .iter()
            .chain(once(&self.panel))
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
        let Some(mut bounds) = viewport.intersection(&layout.bounds()) else {
            return;
        };

        // https://github.com/iced-rs/iced/issues/2700
        if bounds.height < 1.0 {
            return;
        }

        border(renderer, bounds, theme);

        let panel_tree = tree.children.last().unwrap();
        let panel_layout = layout.children().next_back().unwrap();
        let panel_width = panel_layout.bounds().size().width;

        self.panel.as_widget().draw(
            panel_tree,
            renderer,
            theme,
            style,
            panel_layout,
            cursor,
            viewport,
        );

        bounds.width -= panel_width;
        bounds.x += panel_width;

        self.clips
            .iter()
            .zip(&tree.children[1..])
            .zip(layout.children())
            .for_each(|((child, tree), layout)| {
                renderer.with_layer(bounds, |renderer| {
                    child
                        .as_widget()
                        .draw(tree, renderer, theme, style, layout, cursor, &bounds);
                });
            });
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: iced::Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> Status {
        self.panel.as_widget_mut().on_event(
            tree.children.last_mut().unwrap(),
            event,
            layout.children().next_back().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }
}

impl<'a, Message> Track<'a, Message> {
    pub fn new(
        inner: Arc<TrackInner>,
        position: &'a ArrangementPosition,
        scale: &'a ArrangementScale,
        panel: Element<'a, Message>,
    ) -> Self {
        let clips = inner
            .clips()
            .iter()
            .cloned()
            .map(|clip| TrackClip::new(clip, scale))
            .map(Element::new)
            .collect();

        Self {
            inner,
            clips,
            panel,
            position,
            scale,
        }
    }
}

impl TrackExt for TrackInner {
    fn get_clip_at_global_time(
        &self,
        meter: &Meter,
        global_time: usize,
    ) -> Option<Arc<TrackClipInner>> {
        self.clips().iter().rev().find_map(|clip| {
            if clip.get_global_start().in_interleaved_samples(meter) <= global_time
                && global_time <= clip.get_global_end().in_interleaved_samples(meter)
            {
                Some(clip.clone())
            } else {
                None
            }
        })
    }

    fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        viewport: Rectangle,
        position: &ArrangementPosition,
        scale: &ArrangementScale,
    ) -> Vec<Mesh> {
        let meter = self.meter();
        self.clips()
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
}
