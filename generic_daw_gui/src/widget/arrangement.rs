use super::{Vec2, get_time};
use generic_daw_core::{Meter, Position};
use generic_daw_utils::NoDebug;
use iced::{
    Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Renderer as _, Shell,
        layout::{Layout, Limits, Node},
        renderer::Style,
        widget::{Tree, Widget, tree},
    },
    mouse::{self, Cursor, Interaction},
    window,
};

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Grab(usize, usize),
    Drop,
    Clone(usize, usize),
    Drag(usize, Position),
    TrimStart(Position),
    TrimEnd(Position),
    Delete(usize, usize),
}

#[derive(Clone, Copy, Default, PartialEq)]
enum State {
    #[default]
    None,
    DraggingClip(f32, usize, Position),
    ClipTrimmingStart(f32, Position),
    ClipTrimmingEnd(f32, Position),
    DeletingClips,
}

impl State {
    fn unselect(&self) -> bool {
        matches!(
            self,
            Self::DraggingClip(..) | Self::ClipTrimmingStart(..) | Self::ClipTrimmingEnd(..)
        )
    }
}

#[derive(Debug)]
pub struct Arrangement<'a, Message> {
    meter: &'a Meter,
    /// the position of the top left corner of the arrangement viewport
    position: Vec2,
    /// the scale of the arrangement viewport
    scale: Vec2,
    /// column of rows of [track panel, track]
    children: NoDebug<Element<'a, Message>>,
    /// whether we've sent a clip delete message since the last redraw request
    deleted: bool,

    action: fn(Action) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for Arrangement<'_, Message>
where
    Message: Clone,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&*self.children]);
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&*self.children)]
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        Node::with_children(
            limits.max(),
            vec![
                self.children
                    .as_widget()
                    .layout(&mut tree.children[0], renderer, limits),
            ],
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.children.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if let Event::Window(window::Event::RedrawRequested(..)) = event {
            self.deleted = false;
            return;
        }

        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        let Some(cursor) = cursor.position_in(bounds) else {
            if *state != State::None {
                *state = State::None;
                shell.request_redraw();

                if state.unselect() {
                    shell.publish((self.action)(Action::Drop));
                }
            }

            return;
        };

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed { button, modifiers } => match button {
                    mouse::Button::Left => {
                        if let Some((track, clip)) = self.get_track_clip(&layout, cursor) {
                            let time = get_time(
                                cursor.x,
                                *modifiers,
                                self.meter,
                                self.position,
                                self.scale,
                            );

                            let clip_bounds = clip_bounds(&layout, track, clip).unwrap()
                                - Vector::new(bounds.x, bounds.y);

                            let start_pixel = clip_bounds.x;
                            let end_pixel = clip_bounds.x + clip_bounds.width;
                            let offset = start_pixel - cursor.x;

                            *state = match (
                                cursor.x - start_pixel < 10.0,
                                end_pixel - cursor.x < 10.0,
                            ) {
                                (true, true) if cursor.x - start_pixel < end_pixel - cursor.x => {
                                    State::ClipTrimmingStart(offset, time)
                                }
                                (true, false) => State::ClipTrimmingStart(offset, time),
                                (_, true) => {
                                    State::ClipTrimmingEnd(offset + end_pixel - start_pixel, time)
                                }
                                (false, false) => State::DraggingClip(offset, track, time),
                            };

                            shell.publish((self.action)(if modifiers.control() {
                                Action::Clone(track, clip)
                            } else {
                                Action::Grab(track, clip)
                            }));
                            shell.capture_event();
                            shell.request_redraw();
                        }
                    }
                    mouse::Button::Right if !self.deleted => {
                        *state = State::DeletingClips;
                        shell.request_redraw();

                        if let Some((track, clip)) = self.get_track_clip(&layout, cursor) {
                            self.deleted = true;

                            shell.publish((self.action)(Action::Delete(track, clip)));
                            shell.capture_event();
                        }
                    }
                    _ => {}
                },
                mouse::Event::ButtonReleased(..) if *state != State::None => {
                    if state.unselect() {
                        shell.publish((self.action)(Action::Drop));
                    }

                    *state = State::None;
                    shell.capture_event();
                    shell.request_redraw();
                }
                mouse::Event::CursorMoved { modifiers, .. } => match *state {
                    State::DraggingClip(offset, track, time) => {
                        let new_track = self
                            .get_track(cursor.y)
                            .min(layout.children().next().unwrap().children().count() - 1);

                        let new_start = get_time(
                            cursor.x + offset,
                            *modifiers,
                            self.meter,
                            self.position,
                            self.scale,
                        );

                        if new_track != track || new_start != time {
                            *state = State::DraggingClip(offset, new_track, new_start);

                            shell.publish((self.action)(Action::Drag(new_track, new_start)));
                            shell.capture_event();
                        }
                    }
                    State::ClipTrimmingStart(offset, time) => {
                        let new_start = get_time(
                            cursor.x + offset,
                            *modifiers,
                            self.meter,
                            self.position,
                            self.scale,
                        );
                        if new_start != time {
                            *state = State::ClipTrimmingStart(offset, new_start);

                            shell.publish((self.action)(Action::TrimStart(new_start)));
                            shell.capture_event();
                        }
                    }
                    State::ClipTrimmingEnd(offset, time) => {
                        let new_end = get_time(
                            cursor.x + offset,
                            *modifiers,
                            self.meter,
                            self.position,
                            self.scale,
                        );
                        if new_end != time {
                            *state = State::ClipTrimmingEnd(offset, new_end);

                            shell.publish((self.action)(Action::TrimEnd(new_end)));
                            shell.capture_event();
                        }
                    }
                    State::DeletingClips => {
                        if !self.deleted {
                            if let Some((track, clip)) = self.get_track_clip(&layout, cursor) {
                                self.deleted = true;

                                shell.publish((self.action)(Action::Delete(track, clip)));
                                shell.capture_event();
                            }
                        }
                    }
                    State::None => {}
                },
                _ => {}
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> Interaction {
        match tree.state.downcast_ref::<State>() {
            State::ClipTrimmingStart(..) | State::ClipTrimmingEnd(..) => {
                Interaction::ResizingHorizontally
            }
            State::DraggingClip(..) => Interaction::Grabbing,
            State::DeletingClips => Interaction::NoDrop,
            State::None => self.children.as_widget().mouse_interaction(
                &tree.children[0],
                layout.children().next().unwrap(),
                cursor,
                viewport,
                renderer,
            ),
        }
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
        let Some(bounds) = layout.bounds().intersection(viewport) else {
            return;
        };

        renderer.with_layer(bounds, |renderer| {
            self.children.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout.children().next().unwrap(),
                cursor,
                &bounds,
            );
        });
    }
}

impl<'a, Message> Arrangement<'a, Message>
where
    Message: Clone + 'a,
{
    pub fn new(
        meter: &'a Meter,
        position: Vec2,
        scale: Vec2,
        children: impl Into<Element<'a, Message>>,
        action: fn(Action) -> Message,
    ) -> Self {
        Self {
            meter,
            children: children.into().into(),
            position,
            scale,
            deleted: false,
            action,
        }
    }

    fn get_track(&self, y: f32) -> usize {
        (y / self.scale.y) as usize
    }

    fn get_track_clip(&self, layout: &Layout<'_>, cursor: Point) -> Option<(usize, usize)> {
        let track = self.get_track(cursor.y);
        let offset = Vector::new(layout.position().x, layout.position().y);
        let track_layout = track_layout(layout, track)?;
        let clip = track_layout.children().count()
            - track_layout
                .children()
                .rev()
                .position(|l| (l.bounds() - offset).contains(cursor))?
            - 1;
        Some((track, clip))
    }
}

impl<'a, Message> From<Arrangement<'a, Message>> for Element<'a, Message>
where
    Message: Clone + 'a,
{
    fn from(value: Arrangement<'a, Message>) -> Self {
        Self::new(value)
    }
}

fn track_layout<'a>(layout: &Layout<'a>, track: usize) -> Option<Layout<'a>> {
    layout.children().next()?.children().nth(track)
}

fn clip_bounds(layout: &Layout<'_>, track: usize, clip: usize) -> Option<Rectangle> {
    Some(track_layout(layout, track)?.children().nth(clip)?.bounds())
}
