use super::{ArrangementPosition, ArrangementScale, Track, LINE_HEIGHT};
use crate::generic_back::{Arrangement as ArrangementInner, Numerator, Position, TrackClip};
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
    Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
};
use iced_graphics::cache::Group;
use iced_wgpu::{geometry::Cache, graphics::cache::Cached as _, Geometry};
use std::{
    cell::{Cell, RefCell},
    fmt::{Debug, Formatter},
    rc::Rc,
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

struct State<'a, Message> {
    /// the position of the top left corner of the arrangement viewport
    position: Rc<ArrangementPosition>,
    /// information about the scale of the timeline viewport
    scale: Rc<ArrangementScale>,
    /// list of all the track widgets
    tracks: RefCell<Vec<Track<'a, Message>>>,
    /// saves the numerator from the last draw
    numerator: Cell<Numerator>,
    /// caches the meshes of the waveforms
    waveform_cache: RefCell<Option<Cache>>,
    /// the current modifiers
    modifiers: Modifiers,
    /// the current action
    action: Action,
    /// the last window size
    last_bounds: Cell<Option<Rectangle>>,
    /// the theme of the last draw
    last_theme: RefCell<Option<Theme>>,
}

impl<Message> Default for State<'_, Message> {
    fn default() -> Self {
        Self {
            position: Rc::default(),
            scale: Rc::default(),
            tracks: RefCell::default(),
            numerator: Cell::default(),
            waveform_cache: RefCell::default(),
            modifiers: Modifiers::default(),
            action: Action::default(),
            last_bounds: Cell::default(),
            last_theme: RefCell::default(),
        }
    }
}

pub struct Arrangement<'a, Message> {
    inner: Arc<ArrangementInner>,
    /// list of all the track widgets
    tracks: RefCell<Vec<Element<'a, Message, Theme, Renderer>>>,
    /// trigger a redraw
    ping: Message,
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

        self.tracks.borrow_mut().clear();
        self.tracks
            .borrow_mut()
            .extend(state.tracks.borrow().iter().cloned().map(Element::new));

        self.diff(tree);

        let state = tree.state.downcast_ref::<State<'_, Message>>();

        let mut y = state
            .position
            .y
            .get()
            .mul_add(-state.scale.y.get(), LINE_HEIGHT);

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

        if self.inner.tracks.read().unwrap().len() != state.tracks.borrow().len() {
            state.tracks.borrow_mut().clear();
            state
                .tracks
                .borrow_mut()
                .extend(self.inner.tracks.read().unwrap().iter().map(|track| {
                    Track::new(track.clone(), state.position.clone(), state.scale.clone())
                }));

            state.waveform_cache.borrow_mut().take();

            shell.publish(self.ping.clone());
        } else if self.inner.meter.playing.load(SeqCst) {
            shell.publish(self.ping.clone());
        }

        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = modifiers;
            return Status::Ignored;
        }

        let bounds = layout.bounds();

        let Some(pos) = cursor.position_in(bounds) else {
            state.action = Action::None;
            return Status::Ignored;
        };

        if let Some(status) = self.on_event_any_modifiers(state, &event, pos, shell) {
            return status;
        }

        match (
            state.modifiers.command(),
            state.modifiers.shift(),
            state.modifiers.alt(),
        ) {
            (false, false, false) => {
                if let Some(status) = self.on_event_no_modifiers(state, &event, pos, shell) {
                    return status;
                }
            }
            (true, false, false) => {
                if let Some(status) = self.on_event_command(state, &event, pos, shell) {
                    return status;
                }
            }
            (false, true, false) => {
                if let Some(status) = self.on_event_shift(state, &event) {
                    return status;
                }
            }
            (false, false, true) => {
                if let Some(status) = self.on_event_alt(state, &event, pos, shell) {
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
            .is_some_and(|cursor| cursor.y < LINE_HEIGHT)
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
            state.waveform_cache.borrow_mut().take();
            state.numerator.set(self.inner.meter.numerator.load(SeqCst));
        }

        if state
            .last_bounds
            .get()
            .is_none_or(|last_bounds| last_bounds != bounds)
        {
            state.waveform_cache.borrow_mut().take();
            state.last_bounds.set(Some(layout.bounds()));
        }

        if state
            .last_theme
            .borrow()
            .as_ref()
            .is_none_or(|last_theme| last_theme != theme)
        {
            state.waveform_cache.borrow_mut().take();
            state.last_theme.borrow_mut().replace(theme.clone());
        }

        renderer.with_layer(bounds, |renderer| {
            self.grid(renderer, bounds, theme, state);
        });

        {
            let mut bounds = bounds;
            bounds.y += LINE_HEIGHT;
            bounds.height -= LINE_HEIGHT;

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

            if state.waveform_cache.borrow().is_none() {
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

                state.waveform_cache.borrow_mut().replace(
                    Geometry::Live {
                        meshes,
                        images: Vec::new(),
                        text: Vec::new(),
                    }
                    .cache(Group::unique(), None),
                );
            }
        }

        renderer.with_layer(bounds, |renderer| {
            renderer.draw_geometry(Geometry::load(
                state.waveform_cache.borrow().as_ref().unwrap(),
            ));
        });

        renderer.with_layer(bounds, |renderer| {
            self.playhead(renderer, bounds, theme, state);
        });
    }
}

impl<'a, Message> Arrangement<'a, Message>
where
    Message: 'a,
{
    pub fn new(inner: Arc<ArrangementInner>, ping: Message) -> Self {
        Self {
            inner,
            tracks: RefCell::default(),
            ping,
        }
    }

    fn grid(
        &self,
        renderer: &mut Renderer,
        bounds: Rectangle,
        theme: &Theme,
        state: &State<'_, Message>,
    ) {
        let numerator = self.inner.meter.numerator.load(SeqCst);

        let mut beat =
            Position::from_interleaved_samples(state.position.x.get() as usize, &self.inner.meter);
        if beat.sub_quarter_note != 0 {
            beat.sub_quarter_note = 0;
            beat.quarter_note += 1;
        }

        let mut end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * state.scale.x.get().exp2()) as usize,
                &self.inner.meter,
            );
        end_beat.sub_quarter_note = 0;

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

            let x = (beat.in_interleaved_samples_f(&self.inner.meter) - state.position.x.get())
                / state.scale.x.get().exp2();

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

            beat.quarter_note += 1;
        }
    }

    fn playhead(
        &self,
        renderer: &mut Renderer,
        bounds: Rectangle,
        theme: &Theme,
        state: &State<'_, Message>,
    ) {
        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(bounds.position(), Size::new(bounds.width, LINE_HEIGHT)),
                ..Quad::default()
            },
            theme.extended_palette().primary.base.color,
        );

        let x = (self.inner.meter.sample.load(SeqCst) as f32 - state.position.x.get())
            / state.scale.x.get().exp2();

        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(
                    bounds.position() + Vector::new(x, 0.0),
                    Size::new(2.0, bounds.height),
                ),
                ..Quad::default()
            },
            theme.extended_palette().primary.base.color,
        );

        let mut draw_text = |beat: Position, bar: u16| {
            let x = (beat.in_interleaved_samples_f(&self.inner.meter) - state.position.x.get())
                / state.scale.x.get().exp2();

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
            Position::from_interleaved_samples(state.position.x.get() as usize, &self.inner.meter);
        beat = beat.saturating_sub(if state.scale.x.get() > 11.0 {
            Position::new(4 * numerator as u16, 0)
        } else {
            Position::new(numerator as u16, 0)
        });
        beat.sub_quarter_note = 0;

        let mut end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * state.scale.x.get().exp2()) as usize,
                &self.inner.meter,
            );
        end_beat.sub_quarter_note = 0;

        while beat <= end_beat {
            let bar = beat.quarter_note / numerator as u16;

            if state.scale.x.get() > 11f32 {
                if beat.quarter_note % numerator as u16 == 0 && bar % 4 == 0 {
                    draw_text(beat, bar);
                }
            } else if beat.quarter_note % numerator as u16 == 0 {
                draw_text(beat, bar);
            }

            beat.quarter_note += 1;
        }

        Self::border(renderer, bounds, theme);
    }

    fn border(renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
        // I have no clue why we sometimes have to subtract one extra from the y coordinate
        // but it works so I'm not gonna touch it

        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(bounds.position(), Size::new(1.0, bounds.height)),
                ..Quad::default()
            },
            theme.extended_palette().secondary.weak.color,
        );

        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(
                    bounds.position() + Vector::new(0.0, -1.0),
                    Size::new(bounds.width, 1.0),
                ),
                ..Quad::default()
            },
            theme.extended_palette().secondary.weak.color,
        );

        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(
                    bounds.position() + Vector::new(bounds.width - 1.0, 0.0),
                    Size::new(1.0, bounds.height),
                ),
                ..Quad::default()
            },
            theme.extended_palette().secondary.weak.color,
        );

        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(
                    bounds.position() + Vector::new(0.0, bounds.height - 2.0),
                    Size::new(bounds.width, 1.0),
                ),
                ..Quad::default()
            },
            theme.extended_palette().secondary.weak.color,
        );
    }

    #[expect(clippy::too_many_lines)]
    fn on_event_any_modifiers(
        &self,
        state: &mut State<'_, Message>,
        event: &Event,
        cursor: Point,
        shell: &mut Shell<'_, Message>,
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
                            as usize;
                        if !state.modifiers.alt() {
                            time = Position::from_interleaved_samples(time, &self.inner.meter)
                                .snap(state.scale.x.get(), &self.inner.meter)
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
                            .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                            as usize;
                        let mut new_position =
                            Position::from_interleaved_samples(time, &self.inner.meter);

                        if !state.modifiers.alt() {
                            new_position =
                                new_position.snap(state.scale.x.get(), &self.inner.meter);
                        }

                        if new_position != clip.get_global_start() {
                            clip.move_to(new_position);

                            state.waveform_cache.borrow_mut().take();
                            shell.invalidate_layout();
                        }

                        let new_index = ((cursor.y - LINE_HEIGHT) / state.scale.y.get()) as usize;
                        if index != &new_index
                            && new_index < self.inner.tracks.read().unwrap().len()
                            && self.inner.tracks.read().unwrap()[new_index].try_push(clip)
                        {
                            self.inner.tracks.read().unwrap()[*index].remove_clip(clip);

                            state.waveform_cache.borrow_mut().take();
                            shell.invalidate_layout();

                            state.action = Action::DraggingClip(clip.clone(), new_index, *offset);
                        }

                        return Some(Status::Captured);
                    }
                    Action::DeletingClips => {
                        if cursor.y > LINE_HEIGHT {
                            let index = ((cursor.y - LINE_HEIGHT) / state.scale.y.get()) as usize;
                            if index < self.inner.tracks.read().unwrap().len() {
                                let time = cursor
                                    .x
                                    .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                                    as usize;

                                let clip = state.tracks.borrow()[index]
                                    .get_clip_at_global_time(&self.inner.meter, time);

                                if let Some(clip) = clip {
                                    self.inner.tracks.read().unwrap()[index].remove_clip(&clip);

                                    state.waveform_cache.borrow_mut().take();
                                    shell.invalidate_layout();

                                    return Some(Status::Captured);
                                }
                            }
                        }
                    }
                    Action::ClipTrimmingStart(clip, offset) => {
                        let time = (cursor.x + offset)
                            .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                            as usize;

                        let mut new_position =
                            Position::from_interleaved_samples(time, &self.inner.meter);

                        if !state.modifiers.alt() {
                            new_position =
                                new_position.snap(state.scale.x.get(), &self.inner.meter);
                        }

                        if new_position != clip.get_global_start() {
                            clip.trim_start_to(new_position);

                            state.waveform_cache.borrow_mut().take();
                            shell.invalidate_layout();
                        }

                        return Some(Status::Captured);
                    }
                    Action::ClipTrimmingEnd(clip, offset) => {
                        let time = (cursor.x + offset)
                            .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                            as usize;
                        let mut new_position =
                            Position::from_interleaved_samples(time, &self.inner.meter);

                        if !state.modifiers.alt() {
                            new_position =
                                new_position.snap(state.scale.x.get(), &self.inner.meter);
                        }

                        if new_position != clip.get_global_start() {
                            clip.trim_end_to(new_position);

                            state.waveform_cache.borrow_mut().take();
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
        state: &mut State<'_, Message>,
        event: &Event,
        cursor: Point,
        shell: &mut Shell<'_, Message>,
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
                            self.inner.len().in_interleaved_samples_f(&self.inner.meter),
                        );
                    let y = (y / state.scale.y.get())
                        .mul_add(-0.5, state.position.y.get())
                        .clamp(
                            0.0,
                            self.inner.tracks.read().unwrap().len().saturating_sub(1) as f32,
                        );

                    state.position.x.set(x);

                    if (state.position.y.get() - y).abs() * state.scale.y.get() > 1.0 {
                        shell.invalidate_layout();

                        state.position.y.set(y);
                    }

                    state.waveform_cache.borrow_mut().take();
                    shell.invalidate_layout();

                    return Some(Status::Captured);
                }
                mouse::Event::ButtonPressed(button) => match button {
                    mouse::Button::Left => {
                        if let Some(status) = self.lmb_none_or_alt(state, cursor) {
                            return Some(status);
                        }
                    }
                    mouse::Button::Right => {
                        if cursor.y > LINE_HEIGHT {
                            let index = ((cursor.y - LINE_HEIGHT) / state.scale.y.get()) as usize;
                            if index < self.inner.tracks.read().unwrap().len() {
                                let time = cursor
                                    .x
                                    .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                                    as usize;

                                let clip = state.tracks.borrow()[index]
                                    .get_clip_at_global_time(&self.inner.meter, time);

                                if let Some(clip) = clip {
                                    self.inner.tracks.read().unwrap()[index].remove_clip(&clip);

                                    state.waveform_cache.borrow_mut().take();
                                    shell.invalidate_layout();

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
        shell: &mut Shell<'_, Message>,
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
                    state.waveform_cache.borrow_mut().take();
                    shell.invalidate_layout();

                    return Some(Status::Captured);
                }
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if cursor.y > LINE_HEIGHT {
                        let index = ((cursor.y - LINE_HEIGHT) / state.scale.y.get()) as usize;
                        if index < self.inner.tracks.read().unwrap().len() {
                            let time = cursor
                                .x
                                .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                                as usize;

                            let clip = state.tracks.borrow()[index]
                                .get_clip_at_global_time(&self.inner.meter, time);

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

    fn on_event_shift(&self, state: &State<'_, Message>, event: &Event) -> Option<Status> {
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            let x = match delta {
                ScrollDelta::Pixels { x: _, y } => y * 4.0,
                ScrollDelta::Lines { x: _, y } => y * 200.0,
            };

            let x = x
                .mul_add(-state.scale.x.get().exp2(), state.position.x.get())
                .clamp(
                    0.0,
                    self.inner.len().in_interleaved_samples_f(&self.inner.meter),
                );

            state.position.x.set(x);
            state.waveform_cache.borrow_mut().take();

            return Some(Status::Captured);
        }
        None
    }

    fn on_event_alt(
        &self,
        state: &mut State<'_, Message>,
        event: &Event,
        cursor: Point,
        shell: &mut Shell<'_, Message>,
    ) -> Option<Status> {
        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::WheelScrolled { delta } => {
                    let y = match delta {
                        ScrollDelta::Pixels { x: _, y } => y * 0.1,
                        ScrollDelta::Lines { x: _, y } => y * 10.0,
                    };

                    let y = (y + state.scale.y.get()).clamp(2.0 * LINE_HEIGHT, 10.0 * LINE_HEIGHT);

                    if (state.scale.y.get() - y).abs() > 0.1 {
                        shell.invalidate_layout();
                        state.scale.y.set(y);
                    }

                    state.waveform_cache.borrow_mut().take();

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
        if cursor.y < LINE_HEIGHT {
            let mut time = Position::from_interleaved_samples(
                cursor
                    .x
                    .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                    as usize,
                &self.inner.meter,
            );

            if !state.modifiers.alt() {
                time = time.snap(state.scale.x.get(), &self.inner.meter);
            }

            self.inner
                .meter
                .sample
                .store(time.in_interleaved_samples(&self.inner.meter), SeqCst);
            state.action = Action::DraggingPlayhead;

            return Some(Status::Captured);
        }

        let index = ((cursor.y - LINE_HEIGHT) / state.scale.y.get()) as usize;
        if index < self.inner.tracks.read().unwrap().len() {
            let time = cursor
                .x
                .mul_add(state.scale.x.get().exp2(), state.position.x.get())
                as usize;

            let clip =
                state.tracks.borrow()[index].get_clip_at_global_time(&self.inner.meter, time);

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
