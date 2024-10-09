use crate::{
    generic_back::{Arrangement, Numerator, Position, TrackClip},
    generic_front::{Message, TimelinePosition, TimelineScale},
};
use iced::{
    advanced::{
        graphics::geometry::Renderer as _,
        layout::{Layout, Limits, Node},
        renderer::Style,
        widget::{tree, Tree, Widget},
        Clipboard, Renderer as _, Shell,
    },
    event::Status,
    keyboard::{self, Modifiers},
    mouse::{self, Cursor, Interaction, ScrollDelta},
    widget::canvas::{Cache as CanvasCache, Frame, Group, Path, Stroke, Text},
    window, Event, Length, Pixels, Point, Rectangle, Renderer, Size, Theme,
};
use iced_wgpu::{geometry::Cache, graphics::cache::Cached, Geometry};
use std::{
    cell::{Cell, RefCell},
    sync::{atomic::Ordering::SeqCst, Arc},
};

#[derive(Debug, Default)]
enum Action {
    #[default]
    None,
    DraggingPlayhead,
    DraggingClip(Arc<TrackClip>, usize, f32),
    DeletingClips,
    ClipTrimmingStart(Arc<TrackClip>, f32),
    ClipTrimmingEnd(Arc<TrackClip>, f32),
}

#[derive(Debug)]
pub struct State {
    /// information about the position of the timeline viewport
    pub position: TimelinePosition,
    /// information about the scale of the timeline viewport
    pub scale: TimelineScale,
    /// saves what cursor to show
    pub interaction: Interaction,
    /// saves the number of tracks in the arrangement from the last draw
    tracks: Cell<usize>,
    /// saves the numerator from the last draw
    numerator: Cell<Numerator>,
    /// caches the meshes of the waveforms
    waveform_cache: RefCell<Cache>,
    /// caches the geometry of the grid
    grid_cache: CanvasCache,
    /// the current modifiers
    modifiers: Modifiers,
    /// the current action
    action: Action,
}

impl Default for State {
    fn default() -> Self {
        Self {
            position: TimelinePosition::default(),
            scale: TimelineScale::default(),
            interaction: Interaction::default(),
            tracks: Cell::default(),
            numerator: Cell::default(),
            waveform_cache: RefCell::new(Cache {
                meshes: None,
                images: None,
                text: None,
            }),
            grid_cache: CanvasCache::default(),
            modifiers: Modifiers::default(),
            action: Action::default(),
        }
    }
}

impl Widget<Message, Theme, Renderer> for Arc<Arrangement> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        Node::new(limits.max())
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> Status {
        let state = tree.state.downcast_mut::<State>();

        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = modifiers;
            return Status::Ignored;
        }

        if let Event::Window(window::Event::Resized { .. }) = event {
            state.waveform_cache.borrow_mut().meshes = None;
            return Status::Ignored;
        }

        let bounds = layout.bounds();

        let Some(cursor) = cursor.position_in(bounds) else {
            state.action = Action::None;
            state.interaction = Interaction::default();
            return Status::Ignored;
        };

        if let Some(status) = self.on_event_any_modifiers(state, &event, cursor) {
            return status;
        }

        match (
            state.modifiers.command(),
            state.modifiers.shift(),
            state.modifiers.alt(),
        ) {
            (false, false, false) => {
                if let Some(status) = self.on_event_no_modifiers(state, &event, cursor) {
                    return status;
                }
            }
            (true, false, false) => {
                if let Some(status) = self.on_event_command(state, &event, cursor) {
                    return status;
                }
            }
            (false, true, false) => {
                if let Some(status) = self.on_event_shift(state, &event, cursor) {
                    return status;
                }
            }
            (false, false, true) => {
                if let Some(status) = self.on_event_alt(state, &event, cursor) {
                    return status;
                }
            }
            _ => {}
        }
        Status::Ignored
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> Interaction {
        tree.state.downcast_ref::<State>().interaction
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();

        if self.tracks.read().unwrap().len() != state.tracks.get() {
            state.grid_cache.clear();
            state.waveform_cache.borrow_mut().meshes = None;
            state.tracks.set(self.tracks.read().unwrap().len());
        }

        if self.meter.numerator.load(SeqCst) != state.numerator.get() {
            state.grid_cache.clear();
            state.waveform_cache.borrow_mut().meshes = None;
            state.numerator.set(self.meter.numerator.load(SeqCst));
        }

        renderer.with_layer(bounds, |renderer| {
            renderer.draw_geometry(state.grid_cache.draw(renderer, bounds.size(), |frame| {
                frame.with_clip(bounds, |frame| {
                    self.grid(frame, theme, state);
                });
            }));
        });

        {
            let mut bounds = bounds;
            bounds.y += 16.0;
            bounds.height -= 16.0;

            self.tracks
                .read()
                .unwrap()
                .iter()
                .enumerate()
                .for_each(|(i, track)| {
                    let track_bounds = Rectangle::new(
                        Point::new(
                            bounds.x,
                            ((i as f32) - state.position.y).mul_add(state.scale.y, bounds.y),
                        ),
                        Size::new(bounds.width, state.scale.y),
                    );
                    if track_bounds.intersects(&bounds) {
                        track.draw(renderer, theme, track_bounds, bounds, state);
                    }
                });

            let is_empty = state.waveform_cache.borrow().meshes.is_none();
            if is_empty {
                let meshes = self
                    .tracks
                    .read()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .flat_map(|(i, track)| {
                        let track_bounds = Rectangle::new(
                            Point::new(
                                bounds.x,
                                ((i as f32) - state.position.y).mul_add(state.scale.y, bounds.y),
                            ),
                            Size::new(bounds.width, state.scale.y),
                        );
                        if track_bounds.intersects(&bounds) {
                            track.meshes(theme, track_bounds, bounds, state)
                        } else {
                            Vec::new()
                        }
                    })
                    .collect();

                *state.waveform_cache.borrow_mut() = Geometry::Live {
                    meshes,
                    images: Vec::new(),
                    text: Vec::new(),
                }
                .cache(Group::unique(), None);
            }

            renderer.with_layer(bounds, |renderer| {
                renderer.draw_geometry(Geometry::load(&*state.waveform_cache.borrow()));
            });
        }

        renderer.with_layer(bounds, |renderer| {
            self.playhead(renderer, bounds, theme, state);
        });
    }
}

impl Arrangement {
    fn grid(&self, frame: &mut Frame, theme: &Theme, state: &State) {
        let bounds = frame.size();
        let numerator = self.meter.numerator.load(SeqCst);

        let mut beat = Position::from_interleaved_samples(state.position.x as u32, &self.meter);
        if beat.sub_quarter_note != 0 {
            beat.sub_quarter_note = 0;
            beat.quarter_note += 1;
        }

        let mut end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * state.scale.x.exp2()) as u32,
                &self.meter,
            );
        end_beat.sub_quarter_note = 0;

        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            Size::new(bounds.width, 16.0),
            theme.extended_palette().primary.base.color,
        );

        while beat <= end_beat {
            let bar = beat.quarter_note / numerator as u16;
            let color = if state.scale.x > 11f32 {
                if beat.quarter_note % numerator as u16 == 0 {
                    if bar % 4 == 0 {
                        theme.extended_palette().secondary.strong.color
                    } else {
                        theme.extended_palette().secondary.weak.color
                    }
                } else {
                    beat.quarter_note += 1;
                    continue;
                }
            } else if beat.quarter_note % numerator as u16 == 0 {
                theme.extended_palette().secondary.strong.color
            } else {
                theme.extended_palette().secondary.weak.color
            };

            let x = (beat.in_interleaved_samples(&self.meter) as f32 - state.position.x)
                / state.scale.x.exp2();

            let path = Path::new(|path| {
                path.line_to(Point::new(x, 16.0));
                path.line_to(Point::new(x, bounds.height));
            });

            frame.stroke(&path, Stroke::default().with_color(color));

            if state.scale.x > 11f32 {
                if bar % 4 == 0 {
                    let bar = Text {
                        content: format!("{}", bar + 1),
                        position: Point::new(x + 2.0, 2.0),
                        color: theme.extended_palette().secondary.base.text,
                        size: Pixels(12.0),
                        ..Default::default()
                    };
                    frame.fill_text(bar);
                }
            } else if beat.quarter_note % numerator as u16 == 0 {
                let bar = Text {
                    content: format!("{}", bar + 1),
                    position: Point::new(x + 2.0, 2.0),
                    color: theme.extended_palette().secondary.base.text,
                    size: Pixels(12.0),
                    ..Default::default()
                };
                frame.fill_text(bar);
            }

            beat.quarter_note += 1;
        }

        let mut track_line = (-state.position.y)
            .rem_euclid(1.0)
            .mul_add(state.scale.y, 16.0);
        let last_track_line = (self.tracks.read().unwrap().len() as f32 - state.position.y)
            .mul_add(state.scale.y, 16.0);

        while track_line as u32 <= last_track_line as u32 {
            let path = Path::new(|path| {
                path.line_to(Point::new(0.0, track_line));
                path.line_to(Point::new(bounds.width, track_line));
            });

            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(theme.extended_palette().secondary.weak.color)
                    .with_width(1.0),
            );

            track_line += state.scale.y;
        }
    }

    fn playhead(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme, state: &State) {
        let mut frame = Frame::new(renderer, bounds.size());

        let path = Path::new(|path| {
            let x = (self.meter.global_time.load(SeqCst) as f32 - state.position.x)
                / state.scale.x.exp2();
            path.line_to(Point::new(x, 0.0));
            path.line_to(Point::new(x, bounds.height));
        });

        frame.with_clip(bounds, |frame| {
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(theme.extended_palette().primary.base.color)
                    .with_width(2.0),
            );
        });

        renderer.draw_geometry(frame.into_geometry());
    }

    fn interaction(&self, cursor: Point, state: &State) -> Interaction {
        match state.action {
            Action::None => {}
            _ => return state.interaction,
        }

        if cursor.y < 16.0 {
            return Interaction::ResizingHorizontally;
        }
        let index = (cursor.y - 16.0) / state.scale.y;
        if index >= self.tracks.read().unwrap().len() as f32 {
            return Interaction::default();
        }
        self.tracks.read().unwrap()[index as usize]
            .get_clip_at_global_time(
                &self.meter,
                cursor.x.mul_add(state.scale.x.exp2(), state.position.x) as u32,
            )
            .map_or_else(Interaction::default, |clip| {
                let start_pixel = (clip.get_global_start().in_interleaved_samples(&self.meter)
                    as f32
                    - state.position.x)
                    / state.scale.x.exp2();
                let end_pixel = (clip.get_global_end().in_interleaved_samples(&self.meter) as f32
                    - state.position.x)
                    / state.scale.x.exp2();

                match (cursor.x - start_pixel < 10.0, end_pixel - cursor.x < 10.0) {
                    (false, false) => Interaction::Grab,
                    _ => Interaction::ResizingHorizontally,
                }
            })
    }

    fn on_event_any_modifiers(
        &self,
        state: &mut State,
        event: &Event,
        cursor: Point,
    ) -> Option<Status> {
        match event {
            Event::Mouse(event) => match event {
                mouse::Event::ButtonReleased(mouse::Button::Left) => {
                    state.action = Action::None;
                    return Some(Status::Captured);
                }
                mouse::Event::CursorMoved { .. } => match &state.action {
                    Action::DraggingPlayhead => {
                        let mut time =
                            cursor.x.mul_add(state.scale.x.exp2(), state.position.x) as u32;
                        if !state.modifiers.alt() {
                            time = Position::from_interleaved_samples(time, &self.meter)
                                .snap(state.scale.x, &self.meter)
                                .in_interleaved_samples(&self.meter);
                        }

                        self.meter.global_time.store(time, SeqCst);

                        return Some(Status::Captured);
                    }
                    Action::DraggingClip(clip, index, offset) => {
                        let time =
                            (cursor.x + offset).mul_add(state.scale.x.exp2(), state.position.x);
                        let mut new_position =
                            Position::from_interleaved_samples(time as u32, &self.meter);
                        if !state.modifiers.alt() {
                            new_position = new_position.snap(state.scale.x, &self.meter);
                        }
                        if new_position != clip.get_global_start() {
                            clip.move_to(new_position);
                            state.waveform_cache.borrow_mut().meshes = None;
                        }
                        let new_index = ((cursor.y - 16.0) / state.scale.y) as usize;
                        if index != &new_index
                            && new_index < self.tracks.read().unwrap().len()
                            && self.tracks.read().unwrap()[new_index].try_push(clip)
                        {
                            self.tracks.read().unwrap()[*index].remove_clip(clip);

                            state.waveform_cache.borrow_mut().meshes = None;
                            state.action = Action::DraggingClip(clip.clone(), new_index, *offset);
                        }
                        return Some(Status::Captured);
                    }
                    Action::DeletingClips => {
                        if cursor.y > 16.0 {
                            let index = ((cursor.y - 16.0) / state.scale.y) as usize;
                            if index < self.tracks.read().unwrap().len() {
                                let clip = self.tracks.read().unwrap()[index]
                                    .get_clip_at_global_time(
                                        &self.meter,
                                        cursor.x.mul_add(state.scale.x.exp2(), state.position.x)
                                            as u32,
                                    );
                                if let Some(clip) = clip {
                                    self.tracks.read().unwrap()[index].remove_clip(&clip);

                                    state.waveform_cache.borrow_mut().meshes = None;
                                    state.interaction = Interaction::default();

                                    return Some(Status::Captured);
                                }
                            }
                        }
                    }
                    Action::ClipTrimmingStart(clip, offset) => {
                        let time =
                            (cursor.x + offset).mul_add(state.scale.x.exp2(), state.position.x);

                        let mut new_position =
                            Position::from_interleaved_samples(time as u32, &self.meter);
                        if !state.modifiers.alt() {
                            new_position = new_position.snap(state.scale.x, &self.meter);
                        }
                        if new_position != clip.get_global_start() {
                            clip.trim_start_to(new_position);
                            state.waveform_cache.borrow_mut().meshes = None;
                        }
                        return Some(Status::Captured);
                    }
                    Action::ClipTrimmingEnd(clip, offset) => {
                        let time =
                            (cursor.x + offset).mul_add(state.scale.x.exp2(), state.position.x);
                        let mut new_position =
                            Position::from_interleaved_samples(time as u32, &self.meter);
                        if !state.modifiers.alt() {
                            new_position = new_position.snap(state.scale.x, &self.meter);
                        }

                        if new_position != clip.get_global_start() {
                            clip.trim_end_to(new_position);
                            state.waveform_cache.borrow_mut().meshes = None;
                        }
                        return Some(Status::Captured);
                    }
                    Action::None => {}
                },
                _ => {}
            },
            Event::Window(window::Event::RedrawRequested(_)) => {
                state.interaction = self.interaction(cursor, state);
                return Some(Status::Ignored);
            }
            _ => {}
        }
        None
    }

    fn on_event_no_modifiers(
        &self,
        state: &mut State,
        event: &Event,
        cursor: Point,
    ) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::WheelScrolled { delta } => {
                    let (x, y) = match delta {
                        ScrollDelta::Pixels { x, y } => (x * 2.0, y * 4.0),
                        ScrollDelta::Lines { x, y } => (x * 100.0, y * 200.0),
                    };

                    let x = x
                        .mul_add(-state.scale.x.exp2(), state.position.x)
                        .clamp(0.0, self.len().in_interleaved_samples(&self.meter) as f32);
                    let y = (y / state.scale.y).mul_add(-0.5, state.position.y).clamp(
                        0.0,
                        self.tracks.read().unwrap().len().saturating_sub(1) as f32,
                    );

                    state.position.x = x;
                    state.position.y = y;
                    state.waveform_cache.borrow_mut().meshes = None;
                    state.grid_cache.clear();

                    return Some(Status::Captured);
                }
                mouse::Event::ButtonPressed(button) => match button {
                    mouse::Button::Left => {
                        if let Some(status) = self.lmb_none_or_alt(state, cursor) {
                            return Some(status);
                        }
                    }
                    mouse::Button::Right => {
                        if cursor.y > 16.0 {
                            let index = ((cursor.y - 16.0) / state.scale.y) as usize;
                            if index < self.tracks.read().unwrap().len() {
                                let clip = self.tracks.read().unwrap()[index]
                                    .get_clip_at_global_time(
                                        &self.meter,
                                        cursor.x.mul_add(state.scale.x.exp2(), state.position.x)
                                            as u32,
                                    );
                                if let Some(clip) = clip {
                                    self.tracks.read().unwrap()[index].remove_clip(&clip);

                                    state.waveform_cache.borrow_mut().meshes = None;
                                    state.action = Action::DeletingClips;
                                    state.interaction = Interaction::default();

                                    return Some(Status::Captured);
                                }
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        None
    }

    fn on_event_command(&self, state: &mut State, event: &Event, cursor: Point) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::WheelScrolled { delta } => {
                    let x = match delta {
                        ScrollDelta::Pixels { x: _, y } => -y * 0.01,
                        ScrollDelta::Lines { x: _, y } => -y * 0.5,
                    };

                    let x = (x + state.scale.x).clamp(3.0, 12.999_999);

                    state.scale.x = x;
                    state.waveform_cache.borrow_mut().meshes = None;
                    state.grid_cache.clear();

                    return Some(Status::Captured);
                }
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if cursor.y > 16.0 {
                        let index = ((cursor.y - 16.0) / state.scale.y) as usize;
                        if index < self.tracks.read().unwrap().len() {
                            let clip = self.tracks.read().unwrap()[index].get_clip_at_global_time(
                                &self.meter,
                                cursor.x.mul_add(state.scale.x.exp2(), state.position.x) as u32,
                            );
                            if let Some(clip) = clip {
                                let clip = Arc::new((*clip).clone());
                                let offset =
                                    (clip.get_global_start().in_interleaved_samples(&self.meter)
                                        as f32
                                        - state.position.x)
                                        / state.scale.x.exp2()
                                        - cursor.x;

                                self.tracks.read().unwrap()[index].try_push(&clip);

                                state.action = Action::DraggingClip(clip, index, offset);
                                state.interaction = Interaction::Grabbing;

                                return Some(Status::Captured);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn on_event_shift(&self, state: &mut State, event: &Event, _cursor: Point) -> Option<Status> {
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            let x = match delta {
                ScrollDelta::Pixels { x: _, y } => y * 4.0,
                ScrollDelta::Lines { x: _, y } => y * 200.0,
            };

            let x = x
                .mul_add(-state.scale.x.exp2(), state.position.x)
                .clamp(0.0, self.len().in_interleaved_samples(&self.meter) as f32);

            state.position.x = x;
            state.waveform_cache.borrow_mut().meshes = None;
            state.grid_cache.clear();
            return Some(Status::Captured);
        }
        None
    }

    fn on_event_alt(&self, state: &mut State, event: &Event, cursor: Point) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::WheelScrolled { delta } => {
                    let y = match delta {
                        ScrollDelta::Pixels { x: _, y } => y * 0.1,
                        ScrollDelta::Lines { x: _, y } => y * 10.0,
                    };

                    let y = (y + state.scale.y).clamp(36.0, 200.0);

                    state.scale.y = y;
                    state.waveform_cache.borrow_mut().meshes = None;
                    state.grid_cache.clear();
                    return Some(Status::Captured);
                }
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if let Some(status) = self.lmb_none_or_alt(state, cursor) {
                        return Some(status);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn lmb_none_or_alt(&self, state: &mut State, cursor: Point) -> Option<Status> {
        if cursor.y < 16.0 {
            let mut time = Position::from_interleaved_samples(
                cursor.x.mul_add(state.scale.x.exp2(), state.position.x) as u32,
                &self.meter,
            );
            if !state.modifiers.alt() {
                time = time.snap(state.scale.x, &self.meter);
            }
            self.meter
                .global_time
                .store(time.in_interleaved_samples(&self.meter), SeqCst);
            state.action = Action::DraggingPlayhead;
            state.interaction = Interaction::ResizingHorizontally;
            return Some(Status::Captured);
        }
        let index = ((cursor.y - 16.0) / state.scale.y) as usize;
        if index < self.tracks.read().unwrap().len() {
            let clip = self.tracks.read().unwrap()[index].get_clip_at_global_time(
                &self.meter,
                cursor.x.mul_add(state.scale.x.exp2(), state.position.x) as u32,
            );
            if let Some(clip) = clip {
                let offset = (clip.get_global_start().in_interleaved_samples(&self.meter) as f32
                    - state.position.x)
                    / state.scale.x.exp2()
                    - cursor.x;
                let pixel_len = (clip.get_global_end() - clip.get_global_start())
                    .in_interleaved_samples(&self.meter) as f32
                    / state.scale.x.exp2();
                let start_pixel = (clip.get_global_start().in_interleaved_samples(&self.meter)
                    as f32
                    - state.position.x)
                    / state.scale.x.exp2();
                let end_pixel = (clip.get_global_end().in_interleaved_samples(&self.meter) as f32
                    - state.position.x)
                    / state.scale.x.exp2();

                match (cursor.x - start_pixel < 10.0, end_pixel - cursor.x < 10.0) {
                    (true, true) => {
                        state.action = if cursor.x - start_pixel < end_pixel - cursor.x {
                            Action::ClipTrimmingStart(clip, offset)
                        } else {
                            Action::ClipTrimmingEnd(clip, offset + pixel_len)
                        };
                        state.interaction = Interaction::ResizingHorizontally;
                    }
                    (true, false) => {
                        state.action = Action::ClipTrimmingStart(clip, offset);
                        state.interaction = Interaction::ResizingHorizontally;
                    }
                    (false, true) => {
                        state.action = Action::ClipTrimmingEnd(clip, offset + pixel_len);
                        state.interaction = Interaction::ResizingHorizontally;
                    }
                    (false, false) => {
                        state.action = Action::DraggingClip(clip, index, offset);
                        state.interaction = Interaction::Grabbing;
                    }
                }

                return Some(Status::Captured);
            }
        }
        None
    }
}
