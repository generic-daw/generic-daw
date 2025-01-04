use super::Track;
use crate::{
    generic_back::{Arrangement as ArrangementInner, Numerator, Position, TrackClip},
    generic_front::{TimelinePosition, TimelineScale},
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
    Element, Event, Length, Pixels, Point, Rectangle, Renderer, Size, Theme, Vector,
};
use iced_wgpu::{geometry::Cache, graphics::cache::Cached as _, Geometry};
use std::{
    cell::{Cell, RefCell},
    fmt::{Debug, Formatter},
    rc::Rc,
    sync::{atomic::Ordering::SeqCst, Arc},
};

const SEEKER_HEIGHT: f32 = 16.0;

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

struct State<'a, Message> {
    /// information about the position of the timeline viewport
    position: Rc<TimelinePosition>,
    /// information about the scale of the timeline viewport
    scale: Rc<TimelineScale>,
    /// list of all the track widgets
    tracks: RefCell<Vec<Track<'a, Message>>>,
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
    /// the last window size
    last_layout_bounds: RefCell<Option<Rectangle>>,
}

impl<Message> Default for State<'_, Message> {
    fn default() -> Self {
        Self {
            position: Rc::default(),
            scale: Rc::default(),
            tracks: RefCell::default(),
            numerator: Cell::default(),
            waveform_cache: RefCell::new(Cache {
                meshes: None,
                images: None,
                text: None,
            }),
            grid_cache: CanvasCache::default(),
            modifiers: Modifiers::default(),
            action: Action::default(),
            last_layout_bounds: RefCell::default(),
        }
    }
}

pub struct Arrangement<'a, Message> {
    inner: Arc<ArrangementInner>,
    /// list of all the track widgets
    tracks: RefCell<Vec<Element<'a, Message, Theme, Renderer>>>,
}

impl<Message> Debug for Arrangement<'_, Message> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("").field(&self.inner).finish_non_exhaustive()
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Arrangement<'_, Message>
where
    Message: Clone + 'static,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<'_, Message>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Message>::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.tracks.borrow());
    }

    fn children(&self) -> Vec<Tree> {
        self.tracks.borrow().iter().map(Tree::new).collect()
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let state = tree.state.downcast_ref::<State<'_, Message>>();

        if self.inner.tracks.read().unwrap().len() != state.tracks.borrow().len() {
            state.waveform_cache.borrow_mut().meshes = None;

            state.tracks.borrow_mut().clear();
            state
                .tracks
                .borrow_mut()
                .extend(self.inner.tracks.read().unwrap().iter().map(|track| {
                    Track::new(track.clone(), state.position.clone(), state.scale.clone())
                }));
        }

        self.tracks.borrow_mut().extend(
            state
                .tracks
                .borrow()
                .iter()
                .map(|track| track.clone().into()),
        );

        self.diff(tree);

        let state = tree.state.downcast_ref::<State<'_, Message>>();

        let mut y = state
            .position
            .y
            .get()
            .mul_add(-state.scale.y.get(), SEEKER_HEIGHT);

        Node::with_children(
            limits.max(),
            self.tracks
                .borrow()
                .iter()
                .zip(&mut tree.children)
                .map(|(widget, tree)| {
                    widget.as_widget().layout(
                        tree,
                        renderer,
                        &Limits::new(
                            limits.min(),
                            Size::new(limits.max().width, state.scale.y.get()),
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
            .borrow_mut()
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

        let state = tree.state.downcast_mut::<State<'_, Message>>();

        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = modifiers;
            return Status::Ignored;
        }

        let bounds = layout.bounds();

        let Some(pos) = cursor.position_in(bounds) else {
            state.action = Action::None;
            return Status::Ignored;
        };

        if let Some(status) = self.on_event_any_modifiers(state, &event, pos) {
            return status;
        }

        match (
            state.modifiers.command(),
            state.modifiers.shift(),
            state.modifiers.alt(),
        ) {
            (false, false, false) => {
                if let Some(status) = self.on_event_no_modifiers(state, &event, pos) {
                    return status;
                }
            }
            (true, false, false) => {
                if let Some(status) = self.on_event_command(state, &event, pos) {
                    return status;
                }
            }
            (false, true, false) => {
                if let Some(status) = self.on_event_shift(state, &event, pos) {
                    return status;
                }
            }
            (false, false, true) => {
                if let Some(status) = self.on_event_alt(state, &event, pos) {
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
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> Interaction {
        let state = tree.state.downcast_ref::<State<'_, Message>>();

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
            .is_some_and(|cursor| cursor.y < SEEKER_HEIGHT)
        {
            return Interaction::ResizingHorizontally;
        }

        self.tracks
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
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State<'_, Message>>();
        let bounds = layout.bounds();

        if self.inner.meter.numerator.load(SeqCst) != state.numerator.get() {
            state.grid_cache.clear();
            state.waveform_cache.borrow_mut().meshes = None;
            state.numerator.set(self.inner.meter.numerator.load(SeqCst));
        }

        if let Some(last_layout_bounds) = *state.last_layout_bounds.borrow() {
            if last_layout_bounds != layout.bounds() {
                state.waveform_cache.borrow_mut().meshes = None;
            }
        } else {
            state.waveform_cache.borrow_mut().meshes = None;
        }
        *state.last_layout_bounds.borrow_mut() = Some(layout.bounds());

        renderer.with_layer(bounds, |renderer| {
            renderer.draw_geometry(state.grid_cache.draw(renderer, bounds.size(), |frame| {
                frame.with_clip(bounds, |frame| {
                    self.grid(frame, theme, state);
                });
            }));
        });

        {
            let mut bounds = bounds;
            bounds.y += SEEKER_HEIGHT;
            bounds.height -= SEEKER_HEIGHT;

            self.tracks
                .borrow()
                .iter()
                .zip(&tree.children)
                .zip(layout.children())
                .for_each(|((child, tree), layout)| {
                    child
                        .as_widget()
                        .draw(tree, renderer, theme, style, layout, cursor, &bounds);
                });

            let is_empty = state.waveform_cache.borrow().meshes.is_none();
            if is_empty {
                let meshes = state
                    .tracks
                    .borrow()
                    .iter()
                    .enumerate()
                    .flat_map(|(i, track)| {
                        let track_bounds = Rectangle::new(
                            Point::new(
                                bounds.x,
                                ((i as f32) - state.position.y.get())
                                    .mul_add(state.scale.y.get(), bounds.y),
                            ),
                            Size::new(bounds.width, state.scale.y.get()),
                        );
                        if track_bounds.intersects(&bounds) {
                            track.meshes(theme, track_bounds, bounds, &state.position, &state.scale)
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

impl<'a, Message> Arrangement<'a, Message>
where
    Message: 'a,
{
    pub fn new(inner: Arc<ArrangementInner>) -> Self {
        Self {
            inner,
            tracks: RefCell::default(),
        }
    }

    fn grid(&self, frame: &mut Frame, theme: &Theme, state: &State<'_, Message>) {
        let bounds = frame.size();
        let numerator = self.inner.meter.numerator.load(SeqCst);

        let mut beat =
            Position::from_interleaved_samples(state.position.x.get() as u32, &self.inner.meter);
        if beat.sub_quarter_note != 0 {
            beat.sub_quarter_note = 0;
            beat.quarter_note += 1;
        }

        let mut end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * state.scale.x.get().exp2()) as u32,
                &self.inner.meter,
            );
        end_beat.sub_quarter_note = 0;

        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            Size::new(bounds.width, SEEKER_HEIGHT),
            theme.extended_palette().primary.base.color,
        );

        while beat <= end_beat {
            let bar = beat.quarter_note / numerator as u16;
            let color = if state.scale.x.get() > 11f32 {
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

            let x = (beat.in_interleaved_samples(&self.inner.meter) as f32
                - state.position.x.get())
                / state.scale.x.get().exp2();

            let path = Path::new(|path| {
                path.line_to(Point::new(x, SEEKER_HEIGHT));
                path.line_to(Point::new(x, bounds.height));
            });

            frame.stroke(&path, Stroke::default().with_color(color));

            if state.scale.x.get() > 11f32 {
                if bar % 4 == 0 {
                    let bar = Text {
                        content: format!("{}", bar + 1),
                        position: Point::new(x + 2.0, 2.0),
                        color: theme.extended_palette().secondary.base.text,
                        size: Pixels(12.0),
                        ..Text::default()
                    };
                    frame.fill_text(bar);
                }
            } else if beat.quarter_note % numerator as u16 == 0 {
                let bar = Text {
                    content: format!("{}", bar + 1),
                    position: Point::new(x + 2.0, 2.0),
                    color: theme.extended_palette().secondary.base.text,
                    size: Pixels(12.0),
                    ..Text::default()
                };
                frame.fill_text(bar);
            }

            beat.quarter_note += 1;
        }

        let mut track_line = (-state.position.y.get())
            .rem_euclid(1.0)
            .mul_add(state.scale.y.get(), SEEKER_HEIGHT);
        let last_track_line = (self.inner.tracks.read().unwrap().len() as f32
            - state.position.y.get())
        .mul_add(state.scale.y.get(), SEEKER_HEIGHT);

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

            track_line += state.scale.y.get();
        }
    }

    fn playhead(
        &self,
        renderer: &mut Renderer,
        bounds: Rectangle,
        theme: &Theme,
        state: &State<'_, Message>,
    ) {
        let mut frame = Frame::new(renderer, bounds.size());

        let path = Path::new(|path| {
            let x = (self.inner.meter.global_time.load(SeqCst) as f32 - state.position.x.get())
                / state.scale.x.get().exp2();
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

    fn on_event_any_modifiers(
        &self,
        state: &mut State<'_, Message>,
        event: &Event,
        cursor: Point,
    ) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonReleased(mouse::Button::Left) => {
                    state.action = Action::None;
                    return Some(Status::Captured);
                }
                mouse::Event::CursorMoved { .. } => match &state.action {
                    Action::DraggingPlayhead => {
                        let mut time = cursor
                            .x
                            .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                            as u32;
                        if !state.modifiers.alt() {
                            time = Position::from_interleaved_samples(time, &self.inner.meter)
                                .snap(state.scale.x.get(), &self.inner.meter)
                                .in_interleaved_samples(&self.inner.meter);
                        }

                        self.inner.meter.global_time.store(time, SeqCst);

                        return Some(Status::Captured);
                    }
                    Action::DraggingClip(clip, index, offset) => {
                        let time = (cursor.x + offset)
                            .mul_add(state.scale.x.get().exp2(), state.position.x.get());
                        let mut new_position =
                            Position::from_interleaved_samples(time as u32, &self.inner.meter);
                        if !state.modifiers.alt() {
                            new_position =
                                new_position.snap(state.scale.x.get(), &self.inner.meter);
                        }
                        if new_position != clip.get_global_start() {
                            clip.move_to(new_position);
                            state.waveform_cache.borrow_mut().meshes = None;
                        }
                        let new_index = ((cursor.y - SEEKER_HEIGHT) / state.scale.y.get()) as usize;
                        if index != &new_index
                            && new_index < self.inner.tracks.read().unwrap().len()
                            && self.inner.tracks.read().unwrap()[new_index].try_push(clip)
                        {
                            self.inner.tracks.read().unwrap()[*index].remove_clip(clip);

                            state.waveform_cache.borrow_mut().meshes = None;
                            state.action = Action::DraggingClip(clip.clone(), new_index, *offset);
                        }
                        return Some(Status::Captured);
                    }
                    Action::DeletingClips => {
                        if cursor.y > SEEKER_HEIGHT {
                            let index = ((cursor.y - SEEKER_HEIGHT) / state.scale.y.get()) as usize;
                            if index < self.inner.tracks.read().unwrap().len() {
                                let clip = state.tracks.borrow()[index].get_clip_at_global_time(
                                    &self.inner.meter,
                                    cursor
                                        .x
                                        .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                                        as u32,
                                );
                                if let Some(clip) = clip {
                                    self.inner.tracks.read().unwrap()[index].remove_clip(&clip);

                                    state.waveform_cache.borrow_mut().meshes = None;

                                    return Some(Status::Captured);
                                }
                            }
                        }
                    }
                    Action::ClipTrimmingStart(clip, offset) => {
                        let time = (cursor.x + offset)
                            .mul_add(state.scale.x.get().exp2(), state.position.x.get());

                        let mut new_position =
                            Position::from_interleaved_samples(time as u32, &self.inner.meter);
                        if !state.modifiers.alt() {
                            new_position =
                                new_position.snap(state.scale.x.get(), &self.inner.meter);
                        }
                        if new_position != clip.get_global_start() {
                            clip.trim_start_to(new_position);
                            state.waveform_cache.borrow_mut().meshes = None;
                        }
                        return Some(Status::Captured);
                    }
                    Action::ClipTrimmingEnd(clip, offset) => {
                        let time = (cursor.x + offset)
                            .mul_add(state.scale.x.get().exp2(), state.position.x.get());
                        let mut new_position =
                            Position::from_interleaved_samples(time as u32, &self.inner.meter);
                        if !state.modifiers.alt() {
                            new_position =
                                new_position.snap(state.scale.x.get(), &self.inner.meter);
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
            }
        }
        None
    }

    fn on_event_no_modifiers(
        &self,
        state: &mut State<'_, Message>,
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
                        .mul_add(-state.scale.x.get().exp2(), state.position.x.get())
                        .clamp(
                            0.0,
                            self.inner.len().in_interleaved_samples(&self.inner.meter) as f32,
                        );
                    let y = (y / state.scale.y.get())
                        .mul_add(-0.5, state.position.y.get())
                        .clamp(
                            0.0,
                            self.inner.tracks.read().unwrap().len().saturating_sub(1) as f32,
                        );

                    state.position.x.set(x);
                    state.position.y.set(y);
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
                        if cursor.y > SEEKER_HEIGHT {
                            let index = ((cursor.y - SEEKER_HEIGHT) / state.scale.y.get()) as usize;
                            if index < self.inner.tracks.read().unwrap().len() {
                                let clip = state.tracks.borrow()[index].get_clip_at_global_time(
                                    &self.inner.meter,
                                    cursor
                                        .x
                                        .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                                        as u32,
                                );
                                if let Some(clip) = clip {
                                    self.inner.tracks.read().unwrap()[index].remove_clip(&clip);

                                    state.waveform_cache.borrow_mut().meshes = None;
                                    state.action = Action::DeletingClips;

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

    fn on_event_command(
        &self,
        state: &mut State<'_, Message>,
        event: &Event,
        cursor: Point,
    ) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::WheelScrolled { delta } => {
                    let x = match delta {
                        ScrollDelta::Pixels { x: _, y } => -y * 0.01,
                        ScrollDelta::Lines { x: _, y } => -y * 0.5,
                    };

                    let x = (x + state.scale.x.get()).clamp(3.0, 12.999_999);

                    let cursor_content_x = cursor
                        .x
                        .mul_add(state.scale.x.get().exp2(), state.position.x.get());

                    state
                        .position
                        .x
                        .set(cursor.x.mul_add(-x.exp2(), cursor_content_x).max(0.0));
                    state.scale.x.set(x);
                    state.waveform_cache.borrow_mut().meshes = None;
                    state.grid_cache.clear();

                    return Some(Status::Captured);
                }
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if cursor.y > SEEKER_HEIGHT {
                        let index = ((cursor.y - SEEKER_HEIGHT) / state.scale.y.get()) as usize;
                        if index < self.inner.tracks.read().unwrap().len() {
                            let clip = state.tracks.borrow()[index].get_clip_at_global_time(
                                &self.inner.meter,
                                cursor
                                    .x
                                    .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                                    as u32,
                            );
                            if let Some(clip) = clip {
                                let clip = Arc::new((*clip).clone());
                                let offset = (clip
                                    .get_global_start()
                                    .in_interleaved_samples(&self.inner.meter)
                                    as f32
                                    - state.position.x.get())
                                    / state.scale.x.get().exp2()
                                    - cursor.x;

                                self.inner.tracks.read().unwrap()[index].try_push(&clip);

                                state.action = Action::DraggingClip(clip, index, offset);

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

    fn on_event_shift(
        &self,
        state: &State<'_, Message>,
        event: &Event,
        _cursor: Point,
    ) -> Option<Status> {
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            let x = match delta {
                ScrollDelta::Pixels { x: _, y } => y * 4.0,
                ScrollDelta::Lines { x: _, y } => y * 200.0,
            };

            let x = x
                .mul_add(-state.scale.x.get().exp2(), state.position.x.get())
                .clamp(
                    0.0,
                    self.inner.len().in_interleaved_samples(&self.inner.meter) as f32,
                );

            state.position.x.set(x);
            state.waveform_cache.borrow_mut().meshes = None;
            state.grid_cache.clear();
            return Some(Status::Captured);
        }
        None
    }

    fn on_event_alt(
        &self,
        state: &mut State<'_, Message>,
        event: &Event,
        cursor: Point,
    ) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::WheelScrolled { delta } => {
                    let y = match delta {
                        ScrollDelta::Pixels { x: _, y } => y * 0.1,
                        ScrollDelta::Lines { x: _, y } => y * 10.0,
                    };

                    let y = (y + state.scale.y.get()).clamp(36.0, 200.0);

                    state.scale.y.set(y);
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

    fn lmb_none_or_alt(&self, state: &mut State<'_, Message>, cursor: Point) -> Option<Status> {
        if cursor.y < SEEKER_HEIGHT {
            let mut time = Position::from_interleaved_samples(
                cursor
                    .x
                    .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                    as u32,
                &self.inner.meter,
            );
            if !state.modifiers.alt() {
                time = time.snap(state.scale.x.get(), &self.inner.meter);
            }
            self.inner
                .meter
                .global_time
                .store(time.in_interleaved_samples(&self.inner.meter), SeqCst);
            state.action = Action::DraggingPlayhead;
            return Some(Status::Captured);
        }
        let index = ((cursor.y - SEEKER_HEIGHT) / state.scale.y.get()) as usize;
        if index < self.inner.tracks.read().unwrap().len() {
            let clip = state.tracks.borrow()[index].get_clip_at_global_time(
                &self.inner.meter,
                cursor
                    .x
                    .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                    as u32,
            );
            if let Some(clip) = clip {
                let offset = (clip
                    .get_global_start()
                    .in_interleaved_samples(&self.inner.meter)
                    as f32
                    - state.position.x.get())
                    / state.scale.x.get().exp2()
                    - cursor.x;
                let pixel_len = (clip.get_global_end() - clip.get_global_start())
                    .in_interleaved_samples(&self.inner.meter)
                    as f32
                    / state.scale.x.get().exp2();
                let start_pixel = (clip
                    .get_global_start()
                    .in_interleaved_samples(&self.inner.meter)
                    as f32
                    - state.position.x.get())
                    / state.scale.x.get().exp2();
                let end_pixel = (clip
                    .get_global_end()
                    .in_interleaved_samples(&self.inner.meter)
                    as f32
                    - state.position.x.get())
                    / state.scale.x.get().exp2();

                match (cursor.x - start_pixel < 10.0, end_pixel - cursor.x < 10.0) {
                    (true, true) => {
                        state.action = if cursor.x - start_pixel < end_pixel - cursor.x {
                            Action::ClipTrimmingStart(clip, offset)
                        } else {
                            Action::ClipTrimmingEnd(clip, offset + pixel_len)
                        };
                    }
                    (true, false) => {
                        state.action = Action::ClipTrimmingStart(clip, offset);
                    }
                    (false, true) => {
                        state.action = Action::ClipTrimmingEnd(clip, offset + pixel_len);
                    }
                    (false, false) => {
                        state.action = Action::DraggingClip(clip, index, offset);
                    }
                }

                return Some(Status::Captured);
            }
        }
        None
    }
}

impl<'a, Message> From<Arrangement<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'static,
{
    fn from(arrangement_front: Arrangement<'a, Message>) -> Self {
        Self::new(arrangement_front)
    }
}
