use super::{get_time, grid, wheel_scrolled};
use generic_daw_core::{Meter, MidiKey, MidiNote, Position};
use generic_daw_utils::Vec2;
use iced::{
    Background, Color, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme,
    Transformation, Vector,
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
    padding,
    widget::text::{Alignment, LineHeight, Shaping, Wrapping},
    window,
};
use std::sync::{Arc, atomic::Ordering::Acquire};

const PIANO_WIDTH: f32 = 50.0;

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
    pub notes: Arc<Vec<MidiNote>>,
    pub meter: &'a Meter,
    pub position: Vec2,
    pub scale: Vec2,
    /// whether we've sent a clip delete message since the last redraw request
    pub deleted: bool,

    pub select_note: fn(usize) -> Message,
    pub unselect_note: Message,
    pub add_note: fn(MidiKey, Position) -> Message,
    pub clone_note: fn(usize) -> Message,
    pub move_note_to: fn(MidiKey, Position) -> Message,
    pub trim_note_start: fn(Position) -> Message,
    pub trim_note_end: fn(Position) -> Message,
    pub delete_note: fn(usize) -> Message,
    pub position_scale_delta: fn(Vec2, Vec2) -> Message,
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
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        Node::new(limits.max())
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
        _viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(..)) = event {
            self.deleted = false;
            return;
        }

        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        let Some(cursor) = cursor.position_in(bounds.shrink(padding::left(PIANO_WIDTH))) else {
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
                            let key = 119.0 - cursor.y / self.scale.y - self.position.y;
                            let key = MidiKey(key as i8);

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
                        let new_key = 119.0 - cursor.y / self.scale.y - self.position.y;
                        let new_key = MidiKey(new_key as i8);

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
                mouse::Event::WheelScrolled { delta, modifiers } => {
                    wheel_scrolled(
                        delta,
                        *modifiers,
                        cursor,
                        self.scale,
                        shell,
                        self.position_scale_delta,
                    );
                }
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

        let inner_bounds = bounds.shrink(padding::left(PIANO_WIDTH));

        renderer.with_layer(inner_bounds, |renderer| {
            grid(
                renderer,
                inner_bounds,
                theme,
                self.meter,
                self.position,
                self.scale,
            );
        });

        for note in self.notes.iter() {
            renderer.with_layer(inner_bounds, |renderer| {
                renderer.with_translation(
                    Vector::new(inner_bounds.x, inner_bounds.y),
                    |renderer| {
                        self.draw_note(note, renderer, theme);
                    },
                );
            });
        }

        renderer.with_layer(bounds, |renderer| {
            self.draw_piano(renderer, bounds, theme);
        });
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

impl<Message> PianoRoll<'_, Message> {
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
                (127.0 - f32::from(u16::from(note.key)) - self.position.y) * self.scale.y,
            ),
            Size::new(end - start, self.scale.y),
        )
    }

    fn draw_note(&self, note: &MidiNote, renderer: &mut Renderer, theme: &Theme) {
        let note_bounds = self.note_bounds(note);

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
    }

    fn draw_piano(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
        renderer.start_transformation(Transformation::translate(bounds.x, bounds.y));

        let base = self.position.y as usize;
        let offset = self.position.y.fract() * self.scale.y;

        let rows = (bounds.height / self.scale.y) as usize + 1;

        for i in 0..=rows {
            let key = MidiKey(118 - base as i8 - i as i8);

            let note_bounds = Rectangle::new(
                Point::new(0.0, (i as f32).mul_add(self.scale.y, -offset)),
                Size::new(PIANO_WIDTH, self.scale.y),
            );

            renderer.fill_quad(
                Quad {
                    bounds: note_bounds,
                    ..Quad::default()
                },
                if key.is_black() {
                    Background::Color(Color::BLACK)
                } else {
                    Background::Color(Color::WHITE)
                },
            );

            let note_name = Text {
                content: key.to_string(),
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
                if key.is_black() {
                    Color::WHITE
                } else {
                    Color::BLACK
                },
                note_bounds,
            );
        }

        renderer.end_transformation();

        renderer.fill_quad(
            Quad {
                bounds,
                border: border::width(1.0).color(theme.extended_palette().background.strong.color),
                ..Quad::default()
            },
            Background::Color(Color::TRANSPARENT),
        );
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
