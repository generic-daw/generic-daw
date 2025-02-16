use super::{ArrangementPosition, ArrangementScale, Track, LINE_HEIGHT};
use crate::arrangement_view::ArrangementWrapper;
use generic_daw_core::Position;
use iced::{
    advanced::{
        layout::{Layout, Limits, Node},
        renderer::{Quad, Style},
        text::{Renderer as _, Text},
        widget::{tree, Tree, Widget},
        Clipboard, Renderer as _, Shell,
    },
    alignment::{Horizontal, Vertical},
    event::Status,
    keyboard::{self, Modifiers},
    mouse::{self, Cursor, Interaction, ScrollDelta},
    widget::text::{LineHeight, Shaping, Wrapping},
    window, Background, Border, Color, Element, Event, Length, Point, Rectangle, Renderer, Size,
    Theme, Vector,
};
use std::{
    fmt::{Debug, Formatter},
    sync::atomic::Ordering::Acquire,
};

#[derive(Clone, Copy, Default)]
enum Action {
    #[default]
    None,
    DraggingPlayhead,
    DraggingClip(f32),
    DeletingClips,
    ClipTrimmingStart(f32),
    ClipTrimmingEnd(f32),
}

/// scroll wheel clicks -> trackpad scroll pixels
pub const SWM: f32 = LINE_HEIGHT * 2.5;

#[derive(Default)]
struct State {
    /// the current modifiers
    modifiers: Modifiers,
    /// the current action
    action: Action,
    /// we shouldn't send multiple deletion requests per frame
    deleted: bool,
}

pub struct Arrangement<'a, Message> {
    inner: &'a ArrangementWrapper,
    /// list of all the track widgets
    children: Box<[Element<'a, Message, Theme, Renderer>]>,
    /// the position of the top left corner of the arrangement viewport
    position: ArrangementPosition,
    /// the scale of the arrangement viewport
    scale: ArrangementScale,
    seek_to: fn(usize) -> Message,
    select_clip: fn(usize, usize) -> Message,
    unselect_clip: fn() -> Message,
    clone_clip: fn(usize, usize) -> Message,
    move_clip_to: fn(usize, Position) -> Message,
    trim_clip_start: fn(Position) -> Message,
    trim_clip_end: fn(Position) -> Message,
    delete_clip: fn(usize, usize) -> Message,
    position_scale_delta: fn(ArrangementPosition, ArrangementScale) -> Message,
}

impl<Message> Debug for Arrangement<'_, Message> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Arrangement<'_, Message> {
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
        tree.diff_children(&self.children);
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        self.diff(tree);

        let mut y = self.position.y.mul_add(-self.scale.y, LINE_HEIGHT);

        Node::with_children(
            limits.max(),
            self.children
                .iter()
                .zip(&mut tree.children)
                .map(|(widget, tree)| {
                    widget.as_widget().layout(
                        tree,
                        renderer,
                        &Limits::new(limits.min(), Size::new(limits.max().width, self.scale.y)),
                    )
                })
                .map(|node| {
                    let node = node.translate(Vector::new(0.0, y));
                    y += node.bounds().height;
                    node
                })
                .collect(),
        )
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> Status {
        if self
            .children
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
            == Status::Captured
        {
            return Status::Captured;
        }

        let state = tree.state.downcast_mut::<State>();

        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.modifiers = modifiers;
                return Status::Ignored;
            }
            Event::Window(window::Event::RedrawRequested(..)) => {
                state.deleted = false;
                return Status::Ignored;
            }
            _ => {}
        }

        let Some(mut cursor) = cursor.position_in(layout.bounds()) else {
            shell.publish((self.unselect_clip)());
            state.action = Action::None;
            return Status::Ignored;
        };

        if let Some(track) = layout.children().next() {
            let panel_width = track.children().next().unwrap().bounds().width;
            cursor.x -= panel_width;
        }

        if cursor.x < 0.0 {
            shell.publish((self.unselect_clip)());
            state.action = Action::None;
            return Status::Ignored;
        }

        cursor.y -= LINE_HEIGHT;

        match (
            state.modifiers.command(),
            state.modifiers.shift(),
            state.modifiers.alt(),
        ) {
            (false, false, false) => self.on_event_no_modifiers(state, &event, cursor, shell),
            (true, false, false) => self.on_event_command(state, &event, cursor, shell),
            (false, true, false) => self.on_event_shift(&event, shell),
            (false, false, true) => self.on_event_alt(&event, cursor, shell),
            _ => None,
        }
        .or_else(|| self.on_event_any_modifiers(state, &event, cursor, shell))
        .unwrap_or(Status::Ignored)
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
                return Interaction::ResizingHorizontally;
            }
            Action::DraggingClip(..) => return Interaction::Grabbing,
            Action::DraggingPlayhead => return Interaction::ResizingHorizontally,
            _ => {}
        }

        if cursor
            .position_in(layout.bounds())
            .is_some_and(|cursor| cursor.y < LINE_HEIGHT)
        {
            return Interaction::ResizingHorizontally;
        }

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
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        let mut bounds_no_track_panel = bounds;
        if let Some(track) = layout.children().next() {
            let panel_width = track.children().next().unwrap().bounds().width;
            bounds_no_track_panel.x += panel_width;
            bounds_no_track_panel.width -= panel_width;
        }

        let mut bounds_no_seeker = bounds;
        bounds_no_seeker.y += LINE_HEIGHT;
        bounds_no_seeker.height -= LINE_HEIGHT;

        let Some(inner_bounds) = bounds_no_track_panel.intersection(&bounds_no_seeker) else {
            return;
        };

        renderer.with_layer(inner_bounds, |renderer| {
            self.grid(renderer, inner_bounds, theme);
        });

        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .for_each(|((child, tree), layout)| {
                let Some(bounds) = layout.bounds().intersection(&bounds_no_seeker) else {
                    return;
                };

                child
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, &bounds);
            });

        renderer.with_layer(bounds_no_track_panel, |renderer| {
            self.playhead(renderer, bounds_no_track_panel, theme);
        });

        renderer.fill_quad(
            Quad {
                bounds: bounds_no_track_panel,
                border: Border::default()
                    .width(1.0)
                    .color(theme.extended_palette().secondary.weak.color),
                ..Quad::default()
            },
            Background::Color(Color::TRANSPARENT),
        );
    }
}

impl<'a, Message> Arrangement<'a, Message>
where
    Message: 'a,
{
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        inner: &'a ArrangementWrapper,
        position: ArrangementPosition,
        scale: ArrangementScale,
        track_panel: impl Fn(usize, bool) -> Element<'a, Message>,
        seek_to: fn(usize) -> Message,
        select_clip: fn(usize, usize) -> Message,
        unselect_clip: fn() -> Message,
        clone_clip: fn(usize, usize) -> Message,
        move_clip_to: fn(usize, Position) -> Message,
        trim_clip_start: fn(Position) -> Message,
        trim_clip_end: fn(Position) -> Message,
        delete_clip: fn(usize, usize) -> Message,
        position_scale_delta: fn(ArrangementPosition, ArrangementScale) -> Message,
    ) -> Self {
        let children = inner
            .tracks()
            .iter()
            .enumerate()
            .map(|(idx, track)| Track::new(track, position, scale, &track_panel, idx))
            .map(Element::new)
            .collect();

        Self {
            inner,
            children,
            position,
            scale,
            seek_to,
            select_clip,
            unselect_clip,
            clone_clip,
            move_clip_to,
            trim_clip_start,
            trim_clip_end,
            delete_clip,
            position_scale_delta,
        }
    }

    fn grid(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
        let numerator = self.inner.meter.numerator.load(Acquire);

        let mut beat =
            Position::from_interleaved_samples(self.position.x as usize, &self.inner.meter).ceil();

        let end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * self.scale.x.exp2()) as usize,
                &self.inner.meter,
            )
            .floor();

        while beat <= end_beat {
            let bar = beat.quarter_note() / numerator as u32;
            let color = if self.scale.x > 11f32 {
                if beat.quarter_note() % numerator as u32 == 0 {
                    if bar % 4 == 0 {
                        theme.extended_palette().secondary.strong.color
                    } else {
                        theme.extended_palette().secondary.weak.color
                    }
                } else {
                    beat += Position::QUARTER_NOTE;
                    continue;
                }
            } else if beat.quarter_note() % numerator as u32 == 0 {
                theme.extended_palette().secondary.strong.color
            } else {
                theme.extended_palette().secondary.weak.color
            };

            let x = (beat.in_interleaved_samples_f(&self.inner.meter) - self.position.x)
                / self.scale.x.exp2();

            renderer.fill_quad(
                Quad {
                    bounds: Rectangle::new(
                        bounds.position() + Vector::new(x, 0.0),
                        Size::new(1.0, bounds.height),
                    ),
                    ..Quad::default()
                },
                color,
            );

            beat += Position::QUARTER_NOTE;
        }
    }

    fn playhead(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(bounds.position(), Size::new(bounds.width, LINE_HEIGHT)),
                ..Quad::default()
            },
            theme.extended_palette().primary.base.color,
        );

        let x =
            (self.inner.meter.sample.load(Acquire) as f32 - self.position.x) / self.scale.x.exp2();

        if x >= 0.0 {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle::new(
                        bounds.position() + Vector::new(x, 0.0),
                        Size::new(1.5, bounds.height),
                    ),
                    ..Quad::default()
                },
                theme.extended_palette().primary.base.color,
            );
        }

        let mut draw_text = |beat: Position, bar: u32| {
            let x = (beat.in_interleaved_samples_f(&self.inner.meter) - self.position.x)
                / self.scale.x.exp2();

            let bar = Text {
                content: itoa::Buffer::new().format(bar + 1).to_owned(),
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
                bar,
                bounds.position() + Vector::new(x + 1.0, 0.0),
                theme.extended_palette().secondary.base.text,
                bounds,
            );
        };

        let numerator = self.inner.meter.numerator.load(Acquire);

        let mut beat =
            Position::from_interleaved_samples(self.position.x as usize, &self.inner.meter)
                .saturating_sub(if self.scale.x > 11.0 {
                    Position::new(4 * numerator as u32, 0)
                } else {
                    Position::new(numerator as u32, 0)
                })
                .floor();

        let end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * self.scale.x.exp2()) as usize,
                &self.inner.meter,
            )
            .floor();

        while beat <= end_beat {
            let bar = beat.quarter_note() / numerator as u32;

            if self.scale.x > 11f32 {
                if beat.quarter_note() % numerator as u32 == 0 && bar % 4 == 0 {
                    draw_text(beat, bar);
                }
            } else if beat.quarter_note() % numerator as u32 == 0 {
                draw_text(beat, bar);
            }

            beat += Position::QUARTER_NOTE;
        }
    }

    fn on_event_any_modifiers(
        &self,
        state: &mut State,
        event: &Event,
        cursor: Point,
        shell: &mut Shell<'_, Message>,
    ) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if cursor.y < 0.0 {
                        let time = self
                            .get_time(cursor, 0.0, state.modifiers)
                            .in_interleaved_samples(&self.inner.meter);

                        state.action = Action::DraggingPlayhead;

                        shell.publish((self.seek_to)(time));
                        return Some(Status::Captured);
                    }

                    let (track, clip) = self.get_track_clip(cursor)?;

                    let start_pixel = (self.inner.tracks()[track]
                        .get_clip(clip)
                        .get_global_start()
                        .in_interleaved_samples_f(&self.inner.meter)
                        - self.position.x)
                        / self.scale.x.exp2();
                    let end_pixel = (self.inner.tracks()[track]
                        .get_clip(clip)
                        .get_global_end()
                        .in_interleaved_samples_f(&self.inner.meter)
                        - self.position.x)
                        / self.scale.x.exp2();
                    let offset = start_pixel - cursor.x;

                    state.action =
                        match (cursor.x - start_pixel < 10.0, end_pixel - cursor.x < 10.0) {
                            (true, true) if cursor.x - start_pixel < end_pixel - cursor.x => {
                                Action::ClipTrimmingStart(offset)
                            }
                            (true, false) => Action::ClipTrimmingStart(offset),
                            (_, true) => Action::ClipTrimmingEnd(offset + end_pixel - start_pixel),
                            (false, false) => Action::DraggingClip(offset),
                        };

                    shell.publish((self.select_clip)(track, clip));

                    Some(Status::Captured)
                }
                mouse::Event::ButtonReleased(_) => {
                    state.action = Action::None;

                    shell.publish((self.unselect_clip)());
                    Some(Status::Captured)
                }
                mouse::Event::CursorMoved { .. } => match state.action {
                    Action::DraggingPlayhead => {
                        let time = self
                            .get_time(cursor, 0.0, state.modifiers)
                            .in_interleaved_samples(&self.inner.meter);

                        shell.publish((self.seek_to)(time));
                        Some(Status::Captured)
                    }
                    Action::DraggingClip(offset) => {
                        let new_start = self.get_time(cursor, offset, state.modifiers);

                        let new_track = (cursor.y / self.scale.y + self.position.y) as usize;
                        if new_track >= self.children.len() {
                            return None;
                        }

                        shell.publish((self.move_clip_to)(new_track, new_start));
                        Some(Status::Captured)
                    }
                    Action::DeletingClips => {
                        if state.deleted || cursor.y < 0.0 {
                            return None;
                        }

                        let (track, clip) = self.get_track_clip(cursor)?;

                        shell.publish((self.delete_clip)(track, clip));
                        Some(Status::Captured)
                    }
                    Action::ClipTrimmingStart(offset) => {
                        let new_start = self.get_time(cursor, offset, state.modifiers);

                        shell.publish((self.trim_clip_start)(new_start));
                        Some(Status::Captured)
                    }
                    Action::ClipTrimmingEnd(offset) => {
                        let new_end = self.get_time(cursor, offset, state.modifiers);

                        shell.publish((self.trim_clip_end)(new_end));
                        Some(Status::Captured)
                    }
                    Action::None => None,
                },
                _ => None,
            }
        } else {
            None
        }
    }

    fn on_event_no_modifiers(
        &self,
        state: &mut State,
        event: &Event,
        cursor: Point,
        shell: &mut Shell<'_, Message>,
    ) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::WheelScrolled { delta } => {
                    let (x, y) = match *delta {
                        ScrollDelta::Pixels { x, y } => (-x, -y),
                        ScrollDelta::Lines { x, y } => (-x * SWM, -y * SWM),
                    };
                    let (x, y) = (x * 2.0 * self.scale.x.exp2(), y * 2.0 / self.scale.y);

                    shell.publish((self.position_scale_delta)(
                        ArrangementPosition::new(x, y),
                        ArrangementScale::ZERO,
                    ));

                    Some(Status::Captured)
                }
                mouse::Event::ButtonPressed(mouse::Button::Right) => {
                    if cursor.y < 0.0 {
                        return None;
                    }

                    let (track, clip) = self.get_track_clip(cursor)?;

                    state.deleted = true;
                    state.action = Action::DeletingClips;

                    shell.publish((self.delete_clip)(track, clip));
                    Some(Status::Captured)
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn on_event_command(
        &self,
        state: &mut State,
        event: &Event,
        cursor: Point,
        shell: &mut Shell<'_, Message>,
    ) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::WheelScrolled { delta } => {
                    let x = match delta {
                        ScrollDelta::Pixels { y, .. } => -y,
                        ScrollDelta::Lines { y, .. } => -y * SWM,
                    } * 0.01;
                    let x_pos = cursor.x * (self.scale.x.exp2() - (self.scale.x + x).exp2());

                    shell.publish((self.position_scale_delta)(
                        ArrangementPosition::new(x_pos, 0.0),
                        ArrangementScale::new(x, 0.0),
                    ));

                    Some(Status::Captured)
                }
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if cursor.y < 0.0 {
                        return None;
                    }

                    let (track, clip) = self.get_track_clip(cursor)?;

                    let start_pixel = (self.inner.tracks()[track]
                        .get_clip(clip)
                        .get_global_start()
                        .in_interleaved_samples_f(&self.inner.meter)
                        - self.position.x)
                        / self.scale.x.exp2();
                    let offset = start_pixel - cursor.x;

                    state.action = Action::DraggingClip(offset);

                    shell.publish((self.clone_clip)(track, clip));
                    Some(Status::Captured)
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn on_event_shift(&self, event: &Event, shell: &mut Shell<'_, Message>) -> Option<Status> {
        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let x = match delta {
                    ScrollDelta::Pixels { y, .. } => -y,
                    ScrollDelta::Lines { y, .. } => -y * SWM,
                } * 4.0
                    * self.scale.x.exp2();

                shell.publish((self.position_scale_delta)(
                    ArrangementPosition::new(x, 0.0),
                    ArrangementScale::ZERO,
                ));

                Some(Status::Captured)
            }
            _ => None,
        }
    }

    fn on_event_alt(
        &self,
        event: &Event,
        cursor: Point,
        shell: &mut Shell<'_, Message>,
    ) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::WheelScrolled { delta } => {
                    let y = match *delta {
                        ScrollDelta::Pixels { y, .. } => y,
                        ScrollDelta::Lines { y, .. } => y * SWM,
                    } * 0.1;
                    let y_pos = (cursor.y * y) / (self.scale.y * self.scale.y);

                    shell.publish((self.position_scale_delta)(
                        ArrangementPosition::new(0.0, y_pos),
                        ArrangementScale::new(0.0, y),
                    ));

                    Some(Status::Captured)
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn get_track_clip(&self, cursor: Point) -> Option<(usize, usize)> {
        let track = (cursor.y / self.scale.y + self.position.y) as usize;
        let time = cursor.x.mul_add(self.scale.x.exp2(), self.position.x) as usize;
        let clip = self
            .inner
            .tracks()
            .get(track)?
            .get_clip_at_global_time(&self.inner.meter, time)?;

        Some((track, clip))
    }

    fn get_time(&self, cursor: Point, offset: f32, modifiers: Modifiers) -> Position {
        let time = (cursor.x + offset).mul_add(self.scale.x.exp2(), self.position.x);
        let mut time = Position::from_interleaved_samples(time as usize, &self.inner.meter);

        if !modifiers.alt() {
            time = time.snap(self.scale.x, &self.inner.meter);
        }

        time
    }
}

impl<'a, Message> From<Arrangement<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
{
    fn from(arrangement_front: Arrangement<'a, Message>) -> Self {
        Self::new(arrangement_front)
    }
}
