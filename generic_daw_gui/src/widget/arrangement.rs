use super::{
    border, track::TrackExt as _, ArrangementPosition, ArrangementScale, Track, LINE_HEIGHT,
};
use generic_daw_core::{Arrangement as ArrangementInner, Position, TrackClip};
use iced::{
    advanced::{
        graphics::geometry::Renderer as _,
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
    window, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
};
use iced_wgpu::{
    geometry::Cache,
    graphics::cache::{Cached as _, Group},
    Geometry,
};
use std::{
    cell::RefCell,
    fmt::{Debug, Formatter},
    sync::{atomic::Ordering::SeqCst, Arc},
};

#[derive(Default)]
enum Action {
    #[default]
    None,
    DraggingPlayhead,
    DraggingClip(Arc<TrackClip>, usize, f32),
    DeletingClips,
    ClipTrimmingStart(Arc<TrackClip>, f32),
    ClipTrimmingEnd(Arc<TrackClip>, f32),
}

/// scroll wheel clicks -> trackpad scroll pixels
pub const SWM: f32 = LINE_HEIGHT * 2.5;

#[derive(Default)]
struct State {
    /// caches the meshes of the waveforms
    waveform_cache: RefCell<Option<Cache>>,
    /// the current modifiers
    modifiers: Modifiers,
    /// the current action
    action: Action,
    /// the window size from the last draw
    last_bounds: Option<Rectangle>,
    /// the bpm from the last draw
    last_bpm: u16,
    /// the number of tracks from the last draw
    last_track_count: usize,
    /// the theme from the last draw
    last_theme: RefCell<Option<Theme>>,
}

pub struct Arrangement<'a, Message> {
    inner: &'a ArrangementInner,
    /// list of all the track widgets
    tracks: Box<[Element<'a, Message, Theme, Renderer>]>,
    /// the position of the top left corner of the arrangement viewport
    position: &'a ArrangementPosition,
    /// the scale of the arrangement viewport
    scale: &'a ArrangementScale,
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
        tree.diff_children(&self.tracks);
    }

    fn children(&self) -> Vec<Tree> {
        self.tracks.iter().map(Tree::new).collect()
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        self.diff(tree);

        let mut y = self
            .position
            .y
            .get()
            .mul_add(-self.scale.y.get(), LINE_HEIGHT);

        Node::with_children(
            limits.max(),
            self.tracks
                .iter()
                .zip(&mut tree.children)
                .map(|(widget, tree)| {
                    widget.as_widget().layout(
                        tree,
                        renderer,
                        &Limits::new(
                            limits.min(),
                            Size::new(limits.max().width, self.scale.y.get()),
                        ),
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
            .tracks
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
        };

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if let Event::Window(window::Event::RedrawRequested(..)) = event {
            let bpm = self.inner.meter.bpm.load(SeqCst);
            if bpm != state.last_bpm {
                state.waveform_cache.take();
                state.last_bpm = bpm;
            }

            if state
                .last_bounds
                .is_none_or(|last_bounds| last_bounds != bounds)
            {
                state.waveform_cache.take();
                state.last_bounds.replace(layout.bounds());
            }

            if self.tracks.len() != state.last_track_count {
                state.waveform_cache.take();
                state.last_track_count = self.tracks.len();
            }

            return Status::Ignored;
        }

        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = modifiers;
            return Status::Ignored;
        }

        let Some(mut cursor) = cursor.position_in(bounds) else {
            state.action = Action::None;
            return Status::Ignored;
        };

        if let Some(track) = layout.children().next() {
            let panel_width = track.children().next_back().unwrap().bounds().width;
            cursor.x -= panel_width;
        }

        if let Some(status) = self.on_event_any_modifiers(state, &event, cursor, shell) {
            return status;
        }

        match (
            state.modifiers.command(),
            state.modifiers.shift(),
            state.modifiers.alt(),
        ) {
            (false, false, false) => self.on_event_no_modifiers(state, &event, cursor, shell),
            (true, false, false) => self.on_event_command(state, &event, cursor, shell),
            (false, true, false) => self.on_event_shift(state, &event, shell),
            (false, false, true) => self.on_event_alt(state, &event, cursor, shell),
            _ => None,
        }
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

        self.tracks
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
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();

        if state
            .last_theme
            .borrow()
            .as_ref()
            .is_none_or(|last_theme| last_theme != theme)
        {
            state.waveform_cache.take();
            state.last_theme.borrow_mut().replace(theme.clone());
        }

        let mut bounds_no_track_panel = bounds;
        if let Some(track) = layout.children().next() {
            let panel_width = track.children().next_back().unwrap().bounds().width;
            bounds_no_track_panel.x += panel_width;
            bounds_no_track_panel.width -= panel_width;
        }

        let mut bounds_no_seeker = bounds;
        bounds_no_seeker.y += LINE_HEIGHT;
        bounds_no_seeker.height -= LINE_HEIGHT;

        let inner_bounds = bounds_no_track_panel
            .intersection(&bounds_no_seeker)
            .unwrap();

        renderer.with_layer(inner_bounds, |renderer| {
            self.grid(renderer, inner_bounds, theme);
        });

        self.tracks
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .for_each(|((child, tree), layout)| {
                let Some(bounds) = layout.bounds().intersection(&bounds_no_seeker) else {
                    return;
                };

                renderer.with_layer(bounds, |renderer| {
                    child
                        .as_widget()
                        .draw(tree, renderer, theme, style, layout, cursor, &bounds);
                });
            });

        if state.waveform_cache.borrow().is_none() {
            let meshes = self
                .inner
                .tracks()
                .iter()
                .zip(layout.children())
                .flat_map(|(track, layout)| {
                    let mut bounds = layout.bounds();
                    let Some(clip_bounds) = bounds.intersection(&inner_bounds) else {
                        return Vec::new();
                    };
                    bounds.x = clip_bounds.x;
                    bounds.width = clip_bounds.width;

                    track.meshes(theme, bounds, clip_bounds, self.position, self.scale)
                })
                .collect();

            state.waveform_cache.borrow_mut().replace(
                Geometry::Live {
                    meshes,
                    images: Vec::new(),
                    text: Vec::new(),
                }
                .cache(Group::unique(), None),
            );
        }

        renderer.with_layer(inner_bounds, |renderer| {
            renderer.draw_geometry(Geometry::load(
                state.waveform_cache.borrow().as_ref().unwrap(),
            ));
        });

        renderer.with_layer(bounds_no_track_panel, |renderer| {
            self.playhead(renderer, bounds_no_track_panel, theme);
            border(renderer, bounds_no_track_panel, theme);
        });
    }
}

impl<'a, Message> Arrangement<'a, Message>
where
    Message: 'a,
{
    pub fn new(
        inner: &'a ArrangementInner,
        position: &'a ArrangementPosition,
        scale: &'a ArrangementScale,
        track_panel: impl Fn(usize) -> Element<'a, Message>,
    ) -> Self {
        let tracks = inner
            .tracks()
            .iter()
            .cloned()
            .enumerate()
            .map(|(idx, track)| Track::new(track, position, scale, (track_panel)(idx)))
            .map(Element::new)
            .collect();

        Self {
            inner,
            tracks,
            position,
            scale,
        }
    }

    fn grid(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
        let numerator = self.inner.meter.numerator.load(SeqCst);

        let mut beat =
            Position::from_interleaved_samples(self.position.x.get() as usize, &self.inner.meter)
                .ceil();

        let end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * self.scale.x.get().exp2()) as usize,
                &self.inner.meter,
            )
            .floor();

        while beat <= end_beat {
            let bar = beat.quarter_note() / numerator as u32;
            let color = if self.scale.x.get() > 11f32 {
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

            let x = (beat.in_interleaved_samples_f(&self.inner.meter) - self.position.x.get())
                / self.scale.x.get().exp2();

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

        let x = (self.inner.meter.sample.load(SeqCst) as f32 - self.position.x.get())
            / self.scale.x.get().exp2();

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
            let x = (beat.in_interleaved_samples_f(&self.inner.meter) - self.position.x.get())
                / self.scale.x.get().exp2();

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

        let numerator = self.inner.meter.numerator.load(SeqCst);

        let mut beat =
            Position::from_interleaved_samples(self.position.x.get() as usize, &self.inner.meter)
                .saturating_sub(if self.scale.x.get() > 11.0 {
                    Position::new(4 * numerator as u32, 0)
                } else {
                    Position::new(numerator as u32, 0)
                })
                .floor();

        let end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * self.scale.x.get().exp2()) as usize,
                &self.inner.meter,
            )
            .floor();

        while beat <= end_beat {
            let bar = beat.quarter_note() / numerator as u32;

            if self.scale.x.get() > 11f32 {
                if beat.quarter_note() % numerator as u32 == 0 && bar % 4 == 0 {
                    draw_text(beat, bar);
                }
            } else if beat.quarter_note() % numerator as u32 == 0 {
                draw_text(beat, bar);
            }

            beat += Position::QUARTER_NOTE;
        }
    }

    #[expect(clippy::too_many_lines)]
    fn on_event_any_modifiers(
        &self,
        state: &mut State,
        event: &Event,
        cursor: Point,
        shell: &mut Shell<'_, Message>,
    ) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonReleased(_) => {
                    state.action = Action::None;
                    return Some(Status::Captured);
                }
                mouse::Event::CursorMoved { .. } => match &state.action {
                    Action::DraggingPlayhead => {
                        let mut time = cursor
                            .x
                            .mul_add(self.scale.x.get().exp2(), self.position.x.get())
                            as usize;
                        if !state.modifiers.alt() {
                            time = Position::from_interleaved_samples(time, &self.inner.meter)
                                .snap(self.scale.x.get(), &self.inner.meter)
                                .in_interleaved_samples(&self.inner.meter);
                        }

                        if time != self.inner.meter.sample.load(SeqCst) {
                            self.inner.meter.sample.store(time, SeqCst);
                            shell.invalidate_layout();
                        }

                        return Some(Status::Captured);
                    }
                    Action::DraggingClip(clip, index, offset) => {
                        let time = (cursor.x + offset)
                            .mul_add(self.scale.x.get().exp2(), self.position.x.get())
                            as usize;

                        let mut new_start =
                            Position::from_interleaved_samples(time, &self.inner.meter);

                        if !state.modifiers.alt() {
                            new_start = new_start.snap(self.scale.x.get(), &self.inner.meter);
                        }

                        if new_start != clip.get_global_start() {
                            clip.move_to(new_start);
                            state.waveform_cache.take();
                            shell.invalidate_layout();
                        }

                        let new_index = ((cursor.y - LINE_HEIGHT) / self.scale.y.get()) as usize;
                        if *index != new_index && self.inner.tracks().get(new_index)?.try_push(clip)
                        {
                            self.inner.tracks()[*index].remove_clip(clip);
                            state.action = Action::DraggingClip(clip.clone(), new_index, *offset);
                            state.waveform_cache.take();
                            shell.invalidate_layout();
                        }

                        return Some(Status::Captured);
                    }
                    Action::DeletingClips => {
                        if cursor.y < LINE_HEIGHT {
                            return None;
                        }

                        let index = ((cursor.y - LINE_HEIGHT) / self.scale.y.get()
                            + self.position.x.get()) as usize;

                        let time = cursor
                            .x
                            .mul_add(self.scale.x.get().exp2(), self.position.x.get())
                            as usize;

                        let clip = self
                            .inner
                            .tracks()
                            .get(index)?
                            .get_clip_at_global_time(&self.inner.meter, time)?;

                        self.inner.tracks()[index].remove_clip(&clip);
                        state.waveform_cache.take();
                        shell.invalidate_layout();

                        return Some(Status::Captured);
                    }
                    Action::ClipTrimmingStart(clip, offset) => {
                        let time = (cursor.x + offset)
                            .mul_add(self.scale.x.get().exp2(), self.position.x.get())
                            as usize;

                        let mut new_start =
                            Position::from_interleaved_samples(time, &self.inner.meter);

                        if !state.modifiers.alt() {
                            new_start = new_start.snap(self.scale.x.get(), &self.inner.meter);
                        }

                        if new_start != clip.get_global_start() {
                            clip.trim_start_to(new_start);
                            state.waveform_cache.take();
                            shell.invalidate_layout();
                        }

                        return Some(Status::Captured);
                    }
                    Action::ClipTrimmingEnd(clip, offset) => {
                        let time = (cursor.x + offset)
                            .mul_add(self.scale.x.get().exp2(), self.position.x.get())
                            as usize;

                        let mut new_end =
                            Position::from_interleaved_samples(time, &self.inner.meter);

                        if !state.modifiers.alt() {
                            new_end = new_end.snap(self.scale.x.get(), &self.inner.meter);
                        }

                        if new_end != clip.get_global_end() {
                            clip.trim_end_to(new_end);
                            state.waveform_cache.take();
                            shell.invalidate_layout();
                        }

                        return Some(Status::Captured);
                    }
                    Action::None => {}
                },
                _ => {}
            }
        }
        None
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
                        ScrollDelta::Pixels { x, y } => (-x, y),
                        ScrollDelta::Lines { x, y } => (-x * SWM, y * SWM),
                    };
                    let (x, y) = (x * 2.0, y * 4.0);

                    let x_pos = x
                        .mul_add(self.scale.x.get().exp2(), self.position.x.get())
                        .clamp(
                            0.0,
                            self.inner.len().in_interleaved_samples_f(&self.inner.meter),
                        );
                    let y_pos = (y / self.scale.y.get())
                        .mul_add(-0.5, self.position.y.get())
                        .clamp(0.0, self.inner.tracks().len().saturating_sub(1) as f32);

                    self.position.x.set(x_pos);

                    if (self.position.y.get() - y_pos).abs() * self.scale.y.get() > 1.0 {
                        self.position.y.set(y_pos);
                    }

                    state.waveform_cache.take();
                    shell.invalidate_layout();

                    return Some(Status::Captured);
                }
                mouse::Event::ButtonPressed(button) => match button {
                    mouse::Button::Left => {
                        return self.lmb_default(state, cursor);
                    }
                    mouse::Button::Right => {
                        if cursor.y < LINE_HEIGHT {
                            return None;
                        }

                        let index = ((cursor.y - LINE_HEIGHT) / self.scale.y.get()
                            + self.position.y.get()) as usize;

                        let time = cursor
                            .x
                            .mul_add(self.scale.x.get().exp2(), self.position.x.get())
                            as usize;

                        let clip = self
                            .inner
                            .tracks()
                            .get(index)?
                            .get_clip_at_global_time(&self.inner.meter, time)?;

                        self.inner.tracks()[index].remove_clip(&clip);

                        state.action = Action::DeletingClips;
                        state.waveform_cache.take();
                        shell.invalidate_layout();

                        return Some(Status::Captured);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        None
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

                    let x_scale = (x + self.scale.x.get()).clamp(3.0, 12.999_999);
                    let x_pos = cursor
                        .x
                        .mul_add(
                            self.scale.x.get().exp2() - x_scale.exp2(),
                            self.position.x.get(),
                        )
                        .clamp(
                            0.0,
                            self.inner.len().in_interleaved_samples_f(&self.inner.meter),
                        );

                    self.position.x.set(x_pos);
                    self.scale.x.set(x_scale);
                    state.waveform_cache.take();
                    shell.invalidate_layout();

                    return Some(Status::Captured);
                }
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if cursor.y < LINE_HEIGHT {
                        return None;
                    }

                    let index = ((cursor.y - LINE_HEIGHT) / self.scale.y.get()
                        + self.position.x.get()) as usize;

                    let time = cursor
                        .x
                        .mul_add(self.scale.x.get().exp2(), self.position.x.get())
                        as usize;

                    let clip = self
                        .inner
                        .tracks()
                        .get(index)?
                        .get_clip_at_global_time(&self.inner.meter, time)?;

                    let clip = Arc::new(clip.as_ref().clone());

                    let offset = (clip
                        .get_global_start()
                        .in_interleaved_samples(&self.inner.meter)
                        as f32
                        - self.position.x.get())
                        / self.scale.x.get().exp2()
                        - cursor.x;

                    let ok = self.inner.tracks()[index].try_push(&clip);
                    debug_assert!(ok);

                    state.action = Action::DraggingClip(clip, index, offset);

                    return Some(Status::Captured);
                }
                _ => {}
            }
        }
        None
    }

    fn on_event_shift(
        &self,
        state: &State,
        event: &Event,
        shell: &mut Shell<'_, Message>,
    ) -> Option<Status> {
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            let x = match delta {
                ScrollDelta::Pixels { y, .. } => -y,
                ScrollDelta::Lines { y, .. } => -y * SWM,
            } * 4.0;

            let x_pos = x
                .mul_add(self.scale.x.get().exp2(), self.position.x.get())
                .clamp(
                    0.0,
                    self.inner.len().in_interleaved_samples_f(&self.inner.meter),
                );

            self.position.x.set(x_pos);
            state.waveform_cache.take();
            shell.invalidate_layout();

            return Some(Status::Captured);
        }
        None
    }

    fn on_event_alt(
        &self,
        state: &mut State,
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

                    let y_scale =
                        (y + self.scale.y.get()).clamp(2.0 * LINE_HEIGHT, 10.0 * LINE_HEIGHT);

                    if (self.scale.y.get() - y_scale).abs() > 0.1 {
                        self.scale.y.set(y_scale);
                        state.waveform_cache.take();
                        shell.invalidate_layout();
                    }

                    return Some(Status::Captured);
                }
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    return self.lmb_default(state, cursor);
                }
                _ => {}
            }
        }
        None
    }

    fn lmb_default(&self, state: &mut State, cursor: Point) -> Option<Status> {
        if cursor.y < LINE_HEIGHT {
            let mut time = Position::from_interleaved_samples(
                cursor
                    .x
                    .mul_add(self.scale.x.get().exp2(), self.position.x.get())
                    as usize,
                &self.inner.meter,
            );

            if !state.modifiers.alt() {
                time = time.snap(self.scale.x.get(), &self.inner.meter);
            }

            self.inner
                .meter
                .sample
                .store(time.in_interleaved_samples(&self.inner.meter), SeqCst);
            state.action = Action::DraggingPlayhead;

            return Some(Status::Captured);
        }

        let index =
            ((cursor.y - LINE_HEIGHT) / self.scale.y.get() + self.position.y.get()) as usize;

        let time = cursor
            .x
            .mul_add(self.scale.x.get().exp2(), self.position.x.get()) as usize;

        let clip = self
            .inner
            .tracks()
            .get(index)?
            .get_clip_at_global_time(&self.inner.meter, time)?;

        let offset = (clip
            .get_global_start()
            .in_interleaved_samples(&self.inner.meter) as f32
            - self.position.x.get())
            / self.scale.x.get().exp2()
            - cursor.x;
        let pixel_len =
            clip.len().in_interleaved_samples(&self.inner.meter) as f32 / self.scale.x.get().exp2();

        let start_pixel = (clip
            .get_global_start()
            .in_interleaved_samples(&self.inner.meter) as f32
            - self.position.x.get())
            / self.scale.x.get().exp2();
        let end_pixel = (clip
            .get_global_end()
            .in_interleaved_samples(&self.inner.meter) as f32
            - self.position.x.get())
            / self.scale.x.get().exp2();

        state.action = match (cursor.x - start_pixel < 10.0, end_pixel - cursor.x < 10.0) {
            (true, true) if cursor.x - start_pixel < end_pixel - cursor.x => {
                Action::ClipTrimmingStart(clip, offset)
            }
            (_, true) => Action::ClipTrimmingEnd(clip, offset + pixel_len),
            (true, false) => Action::ClipTrimmingStart(clip, offset),
            (false, false) => Action::DraggingClip(clip, index, offset),
        };

        Some(Status::Captured)
    }
}

impl<'a, Message> From<Arrangement<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'static,
{
    fn from(arrangement_front: Arrangement<'a, Message>) -> Self {
        Self::new(arrangement_front)
    }
}
