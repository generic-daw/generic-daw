use super::{ArrangementPosition, ArrangementScale, TrackClip};
use generic_daw_core::{Meter, Track as TrackInner};
use iced::{
    advanced::{
        layout::{Limits, Node},
        renderer::Style,
        widget::Tree,
        Clipboard, Layout, Renderer as _, Shell, Widget,
    },
    event::Status,
    mouse::{Cursor, Interaction},
    Element, Length, Rectangle, Renderer, Size, Theme, Vector,
};
use std::{iter::once, sync::Arc};

mod track_ext;

pub use track_ext::TrackExt;

pub struct Track<'a, Message> {
    inner: Arc<TrackInner>,
    /// list of the track panel and all the clip widgets
    children: Box<[Element<'a, Message, Theme, Renderer>]>,
    /// the position of the top left corner of the arrangement viewport
    position: ArrangementPosition,
    /// the scale of the arrangement viewport
    scale: ArrangementScale,
}

impl<Message> Widget<Message, Theme, Renderer> for Track<'_, Message> {
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fixed(self.scale.y.floor()),
        }
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        self.diff(tree);

        let meter = self.inner.meter();

        let panel_layout =
            self.children[0]
                .as_widget()
                .layout(&mut tree.children[0], renderer, limits);
        let panel_width = panel_layout.size().width;

        Node::with_children(
            limits.max(),
            once(panel_layout)
                .chain(
                    self.children
                        .iter()
                        .zip(&mut tree.children)
                        .skip(1)
                        .map(|(widget, tree)| {
                            widget.as_widget().layout(
                                tree,
                                renderer,
                                &Limits::new(
                                    limits.min(),
                                    Size::new(f32::INFINITY, limits.max().height),
                                ),
                            )
                        })
                        .zip(self.inner.clips().iter())
                        .map(|(node, clip)| {
                            node.translate(Vector::new(
                                panel_width
                                    + (clip.get_global_start().in_interleaved_samples_f(meter)
                                        - self.position.x)
                                        / self.scale.x.exp2(),
                                0.0,
                            ))
                        }),
                )
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
        self.children
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

        let track_panel_layout = layout.children().next().unwrap();
        let Some(track_panel_bounds) = track_panel_layout.bounds().intersection(viewport) else {
            return;
        };
        let track_panel_width = track_panel_bounds.width;

        renderer.with_layer(track_panel_bounds, |renderer| {
            self.children[0].as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                track_panel_layout,
                cursor,
                viewport,
            );
        });

        let mut viewport = *viewport;
        viewport.x += track_panel_width;
        viewport.width -= track_panel_width;
        let Some(bounds) = bounds.intersection(&viewport) else {
            return;
        };

        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .skip(1)
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
        self.children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .map(|((child, state), layout)| {
                child.as_widget_mut().on_event(
                    state,
                    event.clone(),
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                )
            })
            .fold(Status::Ignored, Status::merge)
    }
}

impl<'a, Message> Track<'a, Message> {
    pub fn new(
        inner: Arc<TrackInner>,
        position: ArrangementPosition,
        scale: ArrangementScale,
        track_panel: impl Fn(usize, bool) -> Element<'a, Message>,
        index: usize,
    ) -> Self {
        let enabled = inner.get_enabled();

        let children = once(track_panel(index, enabled))
            .chain(
                inner
                    .clips()
                    .iter()
                    .cloned()
                    .map(|clip| TrackClip::new(clip, position, scale, enabled))
                    .map(Element::new),
            )
            .collect();

        Self {
            inner,
            children,
            position,
            scale,
        }
    }
}

impl TrackExt for TrackInner {
    fn get_clip_at_global_time(&self, meter: &Meter, global_time: usize) -> Option<usize> {
        self.clips().iter().enumerate().rev().find_map(|(i, clip)| {
            if clip.get_global_start().in_interleaved_samples(meter) <= global_time
                && global_time <= clip.get_global_end().in_interleaved_samples(meter)
            {
                Some(i)
            } else {
                None
            }
        })
    }
}
