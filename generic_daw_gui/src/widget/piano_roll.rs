use super::get_time;
use generic_daw_core::{Meter, MidiKey, MidiNote, Position};
use generic_daw_utils::Vec2;
use iced::{
    Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Text, Widget,
        layout::{Limits, Node},
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::{Tree, tree},
    },
    alignment::Vertical,
    border,
    mouse::{self, Cursor, Interaction},
    widget::text::{Alignment, LineHeight, Shaping, Wrapping},
    window,
};
use std::sync::{Arc, atomic::Ordering::Acquire};

#[non_exhaustive]
#[derive(Clone, Copy, Default, PartialEq)]
enum Action {
    #[default]
    None,
    DraggingNote(f32, MidiKey, Position),
    NoteTrimmingStart(f32, Position),
    NoteTrimmingEnd(f32, Position),
    DeletingNotes,
}

impl Action {
    fn unselect(&self) -> bool {
        matches!(
            self,
            Self::DraggingNote(..) | Self::NoteTrimmingStart(..) | Self::NoteTrimmingEnd(..)
        )
    }
}

#[derive(Default)]
struct State {
    action: Action,
    interaction: Interaction,
}

#[derive(Debug)]
pub struct PianoRoll<'a, Message> {
    notes: Arc<Vec<MidiNote>>,
    meter: &'a Meter,
    position: Vec2,
    scale: Vec2,
    // whether we've sent a clip delete message since the last redraw request
    deleted: bool,

    select_note: fn(usize) -> Message,
    unselect_note: Message,
    add_note: fn(MidiKey, Position) -> Message,
    clone_note: fn(usize) -> Message,
    move_note_to: fn(MidiKey, Position) -> Message,
    trim_note_start: fn(Position) -> Message,
    trim_note_end: fn(Position) -> Message,
    delete_note: fn(usize) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for PianoRoll<'_, Message>
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
        Size::new(Length::Fill, Length::Fixed(128.0 * self.scale.y))
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        Node::new(Size::new(limits.max().width, 128.0 * self.scale.y))
    }

    #[expect(clippy::too_many_lines)]
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(..)) = event {
            self.deleted = false;
            return;
        }

        if shell.is_event_captured() {
            return;
        }

        let Some(bounds) = layout.bounds().intersection(viewport) else {
            return;
        };

        let state = tree.state.downcast_mut::<State>();

        let Some(cursor) = cursor.position_in(bounds) else {
            if state.action != Action::None {
                state.action = Action::None;
                shell.request_redraw();

                if state.action.unselect() {
                    shell.publish(self.unselect_note.clone());
                }
            }

            return;
        };

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed { button, modifiers } => match button {
                    mouse::Button::Left => {
                        let time =
                            get_time(cursor.x, *modifiers, self.meter, self.position, self.scale);

                        if let Some(i) = self.get_note(cursor) {
                            let note = &self.notes[i];

                            let note_bounds = self.note_bounds(note);

                            let start_pixel = note_bounds.x;
                            let end_pixel = note_bounds.x + note_bounds.width;
                            let offset = start_pixel - cursor.x;

                            state.action = match (
                                cursor.x - start_pixel < 10.0,
                                end_pixel - cursor.x < 10.0,
                            ) {
                                (true, true) if cursor.x - start_pixel < end_pixel - cursor.x => {
                                    Action::NoteTrimmingStart(offset, time)
                                }
                                (true, false) => Action::NoteTrimmingStart(offset, time),
                                (_, true) => {
                                    Action::NoteTrimmingEnd(offset + end_pixel - start_pixel, time)
                                }
                                (false, false) => Action::DraggingNote(offset, note.key, time),
                            };

                            if modifiers.control() {
                                shell.publish((self.clone_note)(i));
                            } else {
                                shell.publish((self.select_note)(i));
                            }
                        } else {
                            let key = self.get_key(cursor);

                            state.action = Action::DraggingNote(0.0, key, time);

                            shell.publish((self.add_note)(key, time));
                        }

                        shell.capture_event();
                    }
                    mouse::Button::Right if !self.deleted => {
                        state.action = Action::DeletingNotes;

                        if let Some(note) = self.get_note(cursor) {
                            self.deleted = true;

                            shell.publish((self.delete_note)(note));
                            shell.capture_event();
                        }
                    }
                    _ => {}
                },
                mouse::Event::ButtonReleased(..) if state.action != Action::None => {
                    if state.action.unselect() {
                        shell.publish(self.unselect_note.clone());
                    }

                    state.action = Action::None;
                    shell.capture_event();
                }
                mouse::Event::CursorMoved { modifiers, .. } => match state.action {
                    Action::DraggingNote(offset, key, time) => {
                        let new_key = self.get_key(cursor);

                        let new_start = get_time(
                            cursor.x + offset,
                            *modifiers,
                            self.meter,
                            self.position,
                            self.scale,
                        );

                        if new_key != key || new_start != time {
                            state.action = Action::DraggingNote(offset, new_key, new_start);

                            shell.publish((self.move_note_to)(new_key, new_start));
                            shell.capture_event();
                        }
                    }
                    Action::NoteTrimmingStart(offset, time) => {
                        let new_start = get_time(
                            cursor.x + offset,
                            *modifiers,
                            self.meter,
                            self.position,
                            self.scale,
                        );
                        if new_start != time {
                            state.action = Action::NoteTrimmingStart(offset, new_start);

                            shell.publish((self.trim_note_start)(new_start));
                            shell.capture_event();
                        }
                    }
                    Action::NoteTrimmingEnd(offset, time) => {
                        let new_end = get_time(
                            cursor.x + offset,
                            *modifiers,
                            self.meter,
                            self.position,
                            self.scale,
                        );
                        if new_end != time {
                            state.action = Action::NoteTrimmingEnd(offset, new_end);

                            shell.publish((self.trim_note_end)(new_end));
                            shell.capture_event();
                        }
                    }
                    Action::DeletingNotes if !self.deleted => {
                        if let Some(note) = self.get_note(cursor) {
                            self.deleted = true;

                            shell.publish((self.delete_note)(note));
                            shell.capture_event();
                        }
                    }
                    Action::None => {
                        let interaction = self.interaction(cursor);

                        if interaction != state.interaction {
                            state.interaction = interaction;
                            shell.request_redraw();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let Some(bounds) = layout.bounds().intersection(viewport) else {
            return;
        };

        for note in self.notes.iter() {
            self.draw_note(note, renderer, theme, bounds);
        }
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
}

impl<'a, Message> PianoRoll<'a, Message> {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        notes: Arc<Vec<MidiNote>>,
        meter: &'a Meter,
        position: Vec2,
        scale: Vec2,
        select_note: fn(usize) -> Message,
        unselect_note: Message,
        add_note: fn(MidiKey, Position) -> Message,
        clone_note: fn(usize) -> Message,
        move_note_to: fn(MidiKey, Position) -> Message,
        trim_note_start: fn(Position) -> Message,
        trim_note_end: fn(Position) -> Message,
        delete_note: fn(usize) -> Message,
    ) -> Self {
        Self {
            notes,
            meter,
            position,
            scale,
            deleted: false,
            select_note,
            unselect_note,
            add_note,
            clone_note,
            move_note_to,
            trim_note_start,
            trim_note_end,
            delete_note,
        }
    }

    fn get_key(&self, cursor: Point) -> MidiKey {
        let new_key = 128.0 - cursor.y / self.scale.y - self.position.y;
        MidiKey(new_key as u8)
    }

    fn interaction(&self, cursor: Point) -> Interaction {
        for note in self.notes.iter() {
            let note_bounds = self.note_bounds(note);

            if note_bounds.contains(cursor) {
                let x = cursor.x - note_bounds.x;

                if x < 10.0 || note_bounds.width - x < 10.0 {
                    return Interaction::ResizingHorizontally;
                }

                return Interaction::Grab;
            }
        }

        Interaction::default()
    }

    fn get_note(&self, cursor: Point) -> Option<usize> {
        for (i, note) in self.notes.iter().enumerate() {
            if self.note_bounds(note).contains(cursor) {
                return Some(i);
            }
        }

        None
    }

    fn note_bounds(&self, note: &MidiNote) -> Rectangle {
        let sample_size = self.scale.x.exp2();
        let bpm = self.meter.bpm.load(Acquire);

        let start = (note
            .start
            .in_interleaved_samples_f(bpm, self.meter.sample_rate)
            - self.position.x)
            / sample_size;

        let end = (note
            .end
            .in_interleaved_samples_f(bpm, self.meter.sample_rate)
            - self.position.x)
            / sample_size;

        Rectangle::new(
            Point::new(
                start,
                (127.0 - f32::from(note.key.0) - self.position.y) * self.scale.y,
            ),
            Size::new(end - start, self.scale.y),
        )
    }

    fn draw_note(
        &self,
        note: &MidiNote,
        renderer: &mut Renderer,
        theme: &Theme,
        bounds: Rectangle,
    ) {
        let note_bounds =
            self.note_bounds(note) + Vector::new(bounds.position().x, bounds.position().y);

        let Some(note_bounds) = note_bounds.intersection(&bounds) else {
            return;
        };

        renderer.start_layer(note_bounds);

        renderer.fill_quad(
            Quad {
                bounds: note_bounds,
                border: border::width(1.0).color(theme.extended_palette().background.strong.color),
                ..Quad::default()
            },
            theme.extended_palette().primary.weak.color,
        );

        let note_name = Text {
            content: note.key.to_string(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: renderer.default_size(),
            line_height: LineHeight::default(),
            font: renderer.default_font(),
            align_x: Alignment::Left,
            align_y: Vertical::Top,
            shaping: Shaping::Basic,
            wrapping: Wrapping::None,
        };

        renderer.fill_text(
            note_name,
            note_bounds.position() + Vector::new(3.0, 0.0),
            theme.extended_palette().primary.weak.text,
            note_bounds,
        );

        renderer.end_layer();
    }
}

impl<'a, Message> From<PianoRoll<'a, Message>> for Element<'a, Message>
where
    Message: Clone + 'a,
{
    fn from(value: PianoRoll<'a, Message>) -> Self {
        Self::new(value)
    }
}
