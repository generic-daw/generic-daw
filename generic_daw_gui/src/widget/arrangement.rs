use super::{ArrangementPosition, ArrangementScale, LINE_HEIGHT};
use crate::arrangement_view::ArrangementWrapper;
use generic_daw_core::Position;
use iced::{
    Background, Border, Color, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme,
    Vector,
    advanced::{
        Clipboard, Renderer as _, Shell,
        layout::{Layout, Limits, Node},
        renderer::{Quad, Style},
        text::{Renderer as _, Text},
        widget::{Tree, Widget, tree},
    },
    alignment::{Horizontal, Vertical},
    keyboard::Modifiers,
    mouse::{self, Cursor, Interaction, ScrollDelta},
    widget::text::{LineHeight, Shaping, Wrapping},
    window,
};
use std::{
    fmt::{Debug, Formatter},
    sync::atomic::Ordering::Acquire,
};

#[derive(Clone, Copy, Default, PartialEq)]
enum Action {
    #[default]
    None,
    DraggingPlayhead(Position),
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

/// scroll wheel clicks -> trackpad scroll pixels
pub const SWM: f32 = LINE_HEIGHT * 2.5;

#[derive(Default)]
struct State {
    action: Action,
    deleted: bool,
    hovering_seeker: bool,
}

pub struct Arrangement<'a, Message> {
    inner: &'a ArrangementWrapper,
    /// column of rows of [track panel, track]
    children: Element<'a, Message>,
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
        tree.diff_children(&[&self.children]);
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.children)]
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        Node::with_children(
            limits.max(),
            vec![
                self.children
                    .as_widget()
                    .layout(&mut tree.children[0], renderer, limits)
                    .translate(Vector::new(
                        0.0,
                        self.position.y.mul_add(-self.scale.y, LINE_HEIGHT),
                    )),
            ],
        )
    }

    #[expect(clippy::too_many_lines)]
    #[expect(clippy::cognitive_complexity)]
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

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if let Event::Window(window::Event::RedrawRequested(..)) = event {
            state.deleted = false;

            if self.inner.meter.playing.load(Acquire) {
                shell.request_redraw();
            }

            return;
        }

        if shell.is_event_captured() {
            return;
        }

        let Some(mut cursor) = cursor.position_in(bounds).and_then(|mut cursor| {
            track_panel_bounds(&layout)
                .is_none_or(|bounds| {
                    cursor.x -= bounds.width;
                    cursor.x >= 0.0
                })
                .then_some(cursor)
        }) else {
            if state.hovering_seeker {
                state.hovering_seeker = false;
                shell.request_redraw();
            }

            if state.action != Action::None {
                state.action = Action::None;
                shell.request_redraw();

                if state.action.unselect() {
                    shell.publish((self.unselect_clip)());
                }
            }

            return;
        };

        cursor.y -= LINE_HEIGHT;

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed { button, modifiers } => match button {
                    mouse::Button::Left => {
                        let bpm = self.inner.meter.bpm.load(Acquire);
                        let time = self.get_time(cursor, 0.0, *modifiers);

                        if cursor.y < 0.0 {
                            state.action = Action::DraggingPlayhead(time);
                            let time =
                                time.in_interleaved_samples(bpm, self.inner.meter.sample_rate);
                            shell.publish((self.seek_to)(time));
                            shell.capture_event();
                        } else if let Some((track, clip)) = self.get_track_clip(cursor) {
                            let start_pixel = (self.inner.tracks()[track]
                                .get_clip(clip)
                                .get_global_start()
                                .in_interleaved_samples_f(bpm, self.inner.meter.sample_rate)
                                - self.position.x)
                                / self.scale.x.exp2();
                            let end_pixel = (self.inner.tracks()[track]
                                .get_clip(clip)
                                .get_global_end()
                                .in_interleaved_samples_f(bpm, self.inner.meter.sample_rate)
                                - self.position.x)
                                / self.scale.x.exp2();
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
                            shell.publish((self.select_clip)(track, clip));
                            shell.capture_event();
                        } else {
                            match (modifiers.control(), modifiers.shift(), modifiers.alt()) {
                                (true, false, false) if cursor.y >= 0.0 => {
                                    if let Some((track, clip)) = self.get_track_clip(cursor) {
                                        let time = self.inner.tracks()[track]
                                            .get_clip(clip)
                                            .get_global_start();

                                        let start_pixel = (time.in_interleaved_samples_f(
                                            self.inner.meter.bpm.load(Acquire),
                                            self.inner.meter.sample_rate,
                                        ) - self.position.x)
                                            / self.scale.x.exp2();
                                        let offset = start_pixel - cursor.x;

                                        state.action = Action::DraggingClip(offset, track, time);

                                        shell.publish((self.clone_clip)(track, clip));
                                        shell.capture_event();
                                    };
                                }
                                _ => {}
                            }
                        }
                    }
                    mouse::Button::Right => {
                        match (modifiers.control(), modifiers.shift(), modifiers.alt()) {
                            (false, false, false) if !(state.deleted || cursor.y < 0.0) => {
                                state.action = Action::DeletingClips;

                                if let Some((track, clip)) = self.get_track_clip(cursor) {
                                    state.deleted = true;

                                    shell.publish((self.delete_clip)(track, clip));
                                    shell.capture_event();
                                };
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                },
                mouse::Event::ButtonReleased(_) if state.action != Action::None => {
                    if state.action.unselect() {
                        shell.publish((self.unselect_clip)());
                    }

                    state.action = Action::None;
                    shell.capture_event();
                }
                mouse::Event::CursorMoved { modifiers, .. } => match state.action {
                    Action::DraggingPlayhead(time) => {
                        let new_time = self.get_time(cursor, 0.0, *modifiers);
                        if new_time != time {
                            state.action = Action::DraggingPlayhead(new_time);

                            let new_time = new_time.in_interleaved_samples(
                                self.inner.meter.bpm.load(Acquire),
                                self.inner.meter.sample_rate,
                            );

                            shell.publish((self.seek_to)(new_time));
                            shell.capture_event();
                        }
                    }
                    Action::DraggingClip(offset, track, time) => {
                        let new_track = (cursor.y / self.scale.y + self.position.y) as usize;
                        let new_track = new_track.min(self.inner.tracks().len().saturating_sub(1));

                        let new_start = self.get_time(cursor, offset, *modifiers);

                        if new_track != track || new_start != time {
                            state.action = Action::DraggingClip(offset, new_track, new_start);

                            shell.publish((self.move_clip_to)(new_track, new_start));
                            shell.capture_event();
                        }
                    }
                    Action::ClipTrimmingStart(offset, time) => {
                        let new_start = self.get_time(cursor, offset, *modifiers);
                        if new_start != time {
                            state.action = Action::ClipTrimmingStart(offset, new_start);

                            shell.publish((self.trim_clip_start)(new_start));
                            shell.capture_event();
                        }
                    }
                    Action::ClipTrimmingEnd(offset, time) => {
                        let new_end = self.get_time(cursor, offset, *modifiers);
                        if new_end != time {
                            state.action = Action::ClipTrimmingEnd(offset, new_end);

                            shell.publish((self.trim_clip_end)(new_end));
                            shell.capture_event();
                        }
                    }
                    Action::DeletingClips if !(state.deleted || cursor.y < 0.0) => {
                        if let Some((track, clip)) = self.get_track_clip(cursor) {
                            state.deleted = true;

                            shell.publish((self.delete_clip)(track, clip));
                            shell.capture_event();
                        };
                    }
                    Action::None if state.hovering_seeker != (cursor.y <= 0.0) => {
                        state.hovering_seeker ^= true;
                        shell.request_redraw();
                    }
                    _ => {}
                },
                mouse::Event::WheelScrolled { delta, modifiers } => {
                    let (mut x, mut y) = match *delta {
                        ScrollDelta::Pixels { x, y } => (-x, -y),
                        ScrollDelta::Lines { x, y } => (-x * SWM, -y * SWM),
                    };

                    match (modifiers.control(), modifiers.shift(), modifiers.alt()) {
                        (false, false, false) => {
                            (x, y) = (x * 2.0 * self.scale.x.exp2(), y * 2.0 / self.scale.y);

                            shell.publish((self.position_scale_delta)(
                                ArrangementPosition::new(x, y),
                                ArrangementScale::ZERO,
                            ));
                            shell.capture_event();
                        }
                        (true, false, false) => {
                            y /= 128.0;

                            let x_pos =
                                cursor.x * (self.scale.x.exp2() - (self.scale.x + y).exp2());

                            shell.publish((self.position_scale_delta)(
                                ArrangementPosition::new(x_pos, 0.0),
                                ArrangementScale::new(y, 0.0),
                            ));
                            shell.capture_event();
                        }
                        (false, true, false) => {
                            y *= 4.0 * self.scale.x.exp2();

                            shell.publish((self.position_scale_delta)(
                                ArrangementPosition::new(y, 0.0),
                                ArrangementScale::ZERO,
                            ));
                            shell.capture_event();
                        }
                        (false, false, true) => {
                            y /= 8.0;

                            let y_pos = (cursor.y * y) / (self.scale.y * self.scale.y);

                            shell.publish((self.position_scale_delta)(
                                ArrangementPosition::new(0.0, y_pos),
                                ArrangementScale::new(0.0, y),
                            ));
                            shell.capture_event();
                        }
                        _ => {}
                    }
                }
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
            Action::DraggingPlayhead(..) => Interaction::ResizingHorizontally,
            Action::DeletingClips => Interaction::NoDrop,
            Action::None => {
                if state.hovering_seeker {
                    Interaction::ResizingHorizontally
                } else {
                    self.children.as_widget().mouse_interaction(
                        &tree.children[0],
                        layout.children().next().unwrap(),
                        cursor,
                        viewport,
                        renderer,
                    )
                }
            }
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
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        let mut bounds_no_track_panel = bounds;
        if let Some(bounds) = track_bounds(&layout) {
            bounds_no_track_panel.x = bounds.x;
            bounds_no_track_panel.width = bounds.width;
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

        let mut children = layout.children();

        renderer.with_layer(bounds_no_seeker, |renderer| {
            let layout = children.next().unwrap();

            let Some(bounds) = bounds.intersection(&bounds_no_seeker) else {
                return;
            };

            self.children.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                &bounds,
            );
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
        children: impl Into<Element<'a, Message>>,
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
        Self {
            inner,
            children: children.into(),
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
        let bpm = self.inner.meter.bpm.load(Acquire);

        let mut beat = Position::from_interleaved_samples_f(
            self.position.x,
            bpm,
            self.inner.meter.sample_rate,
        )
        .ceil();

        let end_beat = beat
            + Position::from_interleaved_samples_f(
                bounds.width * self.scale.x.exp2(),
                bpm,
                self.inner.meter.sample_rate,
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

            let x = (beat.in_interleaved_samples_f(bpm, self.inner.meter.sample_rate)
                - self.position.x)
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
        let bpm = self.inner.meter.bpm.load(Acquire);

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
            let x = (beat.in_interleaved_samples_f(bpm, self.inner.meter.sample_rate)
                - self.position.x)
                / self.scale.x.exp2();

            let bar = Text {
                content: itoa::Buffer::new().format(bar + 1).to_owned(),
                bounds: Size::new(f32::INFINITY, 0.0),
                size: renderer.default_size(),
                line_height: LineHeight::default(),
                font: renderer.default_font(),
                horizontal_alignment: Horizontal::Left,
                vertical_alignment: Vertical::Top,
                shaping: Shaping::Basic,
                wrapping: Wrapping::None,
            };

            renderer.fill_text(
                bar,
                bounds.position() + Vector::new(x + 1.0, 0.0),
                theme.extended_palette().primary.base.text,
                bounds,
            );
        };

        let numerator = self.inner.meter.numerator.load(Acquire);

        let mut beat = Position::from_interleaved_samples_f(
            self.position.x,
            bpm,
            self.inner.meter.sample_rate,
        )
        .saturating_sub(if self.scale.x > 11.0 {
            Position::new(4 * numerator as u32, 0)
        } else {
            Position::new(numerator as u32, 0)
        })
        .floor();

        let end_beat = beat
            + Position::from_interleaved_samples_f(
                bounds.width * self.scale.x.exp2(),
                bpm,
                self.inner.meter.sample_rate,
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

    fn get_track_clip(&self, cursor: Point) -> Option<(usize, usize)> {
        let track = (cursor.y / self.scale.y + self.position.y) as usize;
        let time = cursor.x.mul_add(self.scale.x.exp2(), self.position.x) as usize;
        let clip = self
            .inner
            .tracks()
            .get(track)?
            .get_clip_at_global_time(time)?;

        Some((track, clip))
    }

    fn get_time(&self, cursor: Point, offset: f32, modifiers: Modifiers) -> Position {
        let time = (cursor.x + offset).mul_add(self.scale.x.exp2(), self.position.x);
        let mut time = Position::from_interleaved_samples_f(
            time,
            self.inner.meter.bpm.load(Acquire),
            self.inner.meter.sample_rate,
        );

        if !modifiers.alt() {
            time = time.snap(self.scale.x, self.inner.meter.numerator.load(Acquire));
        }

        time
    }
}

impl<'a, Message> From<Arrangement<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(arrangement_front: Arrangement<'a, Message>) -> Self {
        Self::new(arrangement_front)
    }
}

fn track_panel_bounds(layout: &Layout<'_>) -> Option<Rectangle> {
    Some(
        layout
            .children()
            .next()?
            .children()
            .next()?
            .children()
            .next()?
            .bounds(),
    )
}

fn track_bounds(layout: &Layout<'_>) -> Option<Rectangle> {
    Some(
        layout
            .children()
            .next()?
            .children()
            .next()?
            .children()
            .next_back()?
            .bounds(),
    )
}
