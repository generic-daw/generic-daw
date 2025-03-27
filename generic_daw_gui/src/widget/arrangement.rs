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

#[non_exhaustive]
#[derive(Clone, Copy, Default, PartialEq)]
enum Action {
    #[default]
    None,
    DraggingClip(f32, usize, Position),
    ClipTrimmingStart(f32, Position),
    ClipTrimmingEnd(f32, Position),
    DeletingClips,
}

impl Action {
    fn unselect(&self) -> bool {
        matches!(
            self,
            Self::DraggingClip(..) | Self::ClipTrimmingStart(..) | Self::ClipTrimmingEnd(..)
        )
    }
}

#[derive(Default)]
struct State {
    action: Action,
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

    select_clip: fn(usize, usize) -> Message,
    unselect_clip: Message,
    clone_clip: fn(usize, usize) -> Message,
    move_clip_to: fn(usize, Position) -> Message,
    trim_clip_start: fn(Position) -> Message,
    trim_clip_end: fn(Position) -> Message,
    delete_clip: fn(usize, usize) -> Message,
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
            if state.action != Action::None {
                state.action = Action::None;
                shell.request_redraw();

                if state.action.unselect() {
                    shell.publish(self.unselect_clip.clone());
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

                            state.action = match (
                                cursor.x - start_pixel < 10.0,
                                end_pixel - cursor.x < 10.0,
                            ) {
                                (true, true) if cursor.x - start_pixel < end_pixel - cursor.x => {
                                    Action::ClipTrimmingStart(offset, time)
                                }
                                (true, false) => Action::ClipTrimmingStart(offset, time),
                                (_, true) => {
                                    Action::ClipTrimmingEnd(offset + end_pixel - start_pixel, time)
                                }
                                (false, false) => Action::DraggingClip(offset, track, time),
                            };

                            if modifiers.control() {
                                shell.publish((self.clone_clip)(track, clip));
                            } else {
                                shell.publish((self.select_clip)(track, clip));
                            }

                            shell.capture_event();
                            shell.request_redraw();
                        }
                    }
                    mouse::Button::Right if !self.deleted => {
                        state.action = Action::DeletingClips;
                        shell.request_redraw();

                        if let Some((track, clip)) = self.get_track_clip(&layout, cursor) {
                            self.deleted = true;

                            shell.publish((self.delete_clip)(track, clip));
                            shell.capture_event();
                        }
                    }
                    _ => {}
                },
                mouse::Event::ButtonReleased(..) if state.action != Action::None => {
                    if state.action.unselect() {
                        shell.publish(self.unselect_clip.clone());
                    }

                    state.action = Action::None;
                    shell.capture_event();
                    shell.request_redraw();
                }
                mouse::Event::CursorMoved { modifiers, .. } => match state.action {
                    Action::DraggingClip(offset, track, time) => {
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
                            state.action = Action::DraggingClip(offset, new_track, new_start);

                            shell.publish((self.move_clip_to)(new_track, new_start));
                            shell.capture_event();
                        }
                    }
                    Action::ClipTrimmingStart(offset, time) => {
                        let new_start = get_time(
                            cursor.x + offset,
                            *modifiers,
                            self.meter,
                            self.position,
                            self.scale,
                        );
                        if new_start != time {
                            state.action = Action::ClipTrimmingStart(offset, new_start);

                            shell.publish((self.trim_clip_start)(new_start));
                            shell.capture_event();
                        }
                    }
                    Action::ClipTrimmingEnd(offset, time) => {
                        let new_end = get_time(
                            cursor.x + offset,
                            *modifiers,
                            self.meter,
                            self.position,
                            self.scale,
                        );
                        if new_end != time {
                            state.action = Action::ClipTrimmingEnd(offset, new_end);

                            shell.publish((self.trim_clip_end)(new_end));
                            shell.capture_event();
                        }
                    }
                    Action::DeletingClips if !self.deleted => {
                        if let Some((track, clip)) = self.get_track_clip(&layout, cursor) {
                            self.deleted = true;

                            shell.publish((self.delete_clip)(track, clip));
                            shell.capture_event();
                        }
                    }
                    _ => {}
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
        let state = tree.state.downcast_ref::<State>();

        match state.action {
            Action::ClipTrimmingStart(..) | Action::ClipTrimmingEnd(..) => {
                Interaction::ResizingHorizontally
            }
            Action::DraggingClip(..) => Interaction::Grabbing,
            Action::DeletingClips => Interaction::NoDrop,
            Action::None => self.children.as_widget().mouse_interaction(
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
        select_clip: fn(usize, usize) -> Message,
        unselect_clip: Message,
        clone_clip: fn(usize, usize) -> Message,
        move_clip_to: fn(usize, Position) -> Message,
        trim_clip_start: fn(Position) -> Message,
        trim_clip_end: fn(Position) -> Message,
        delete_clip: fn(usize, usize) -> Message,
    ) -> Self {
        Self {
            meter,
            children: children.into().into(),
            position,
            scale,
            deleted: false,
            select_clip,
            unselect_clip,
            clone_clip,
            move_clip_to,
            trim_clip_start,
            trim_clip_end,
            delete_clip,
        }
    }

    fn get_track(&self, y: f32) -> usize {
        (y / self.scale.y) as usize
    }

    fn get_track_clip(&self, layout: &Layout<'_>, cursor: Point) -> Option<(usize, usize)> {
        let track = self.get_track(cursor.y);
        let offset = Vector::new(layout.position().x, layout.position().y);
        let clip = track_layout(layout, track)?
            .children()
            .position(|l| (l.bounds() - offset).contains(cursor))?;
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
