use crate::{
    icons::{LUCIDE_FONT, move_vertical, plus},
    stylefns::button_with_base,
    widget::{DragHandle, LINE_HEIGHT},
};
use iced::{
    Alignment, Color, Element, Font, Shrink,
    border::{self, Radius},
    widget::{
        Button, ComboBox, PickList, Scrollable, Space, Text, TextInput, button, center, combo_box,
        container, mouse_area, opaque, pick_list, row,
        scrollable::{self, Direction},
        stack,
        text::Shaping,
        text_input,
    },
};
use std::borrow::Borrow;

pub fn empty_widget() -> Space {
    Space::new(Shrink, Shrink)
}

pub fn icon_button<'a, Message>(t: Text<'a>) -> Button<'a, Message>
where
    Message: Clone + 'a,
{
    button(
        container(t.size(13.0).line_height(1.0))
            .width(13.0)
            .align_x(Alignment::Center),
    )
    .padding(0.0)
}

pub fn modal<'a, Message>(
    base: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
    exit: Message,
) -> impl Into<Element<'a, Message>>
where
    Message: Clone + 'a,
{
    stack![
        base.into(),
        opaque(
            mouse_area(center(opaque(content)).style(|_| {
                container::Style::default().background(Color::BLACK.scale_alpha(0.75))
            }))
            .on_press(exit)
        )
    ]
}

pub fn number_input<'a, Message>(
    current: usize,
    default: usize,
    max_digits: usize,
    drag_update: fn(usize) -> Message,
    text_update: fn(String) -> Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    row![
        DragHandle::new(
            container(move_vertical())
                .style(|t| {
                    container::Style::default()
                        .background(t.extended_palette().background.weak.color)
                        .border(
                            border::width(1.0)
                                .color(t.extended_palette().background.strongest.color),
                        )
                })
                .padding([5.0, 0.0]),
            current,
            default,
            drag_update
        ),
        styled_text_input("", &current.to_string())
            .font(Font::MONOSPACE)
            .width((max_digits as f32).mul_add(10.0, 14.0))
            .on_input(text_update)
    ]
    .into()
}

pub fn round_plus_button<'a, Message>() -> Button<'a, Message>
where
    Message: Clone + 'a,
{
    button(
        container(plus().size(LINE_HEIGHT + 6.0))
            .width(LINE_HEIGHT + 6.0)
            .align_x(Alignment::Center),
    )
    .style(|t, s| {
        let mut style = button_with_base(t, s, button::primary);
        style.border.radius = f32::INFINITY.into();
        style
    })
    .padding(5.0)
}

pub fn styled_button<'a, Message>(content: impl Into<Element<'a, Message>>) -> Button<'a, Message> {
    button(content)
        .style(|t, s| button_with_base(t, s, button::primary))
        .padding([5.0, 7.0])
}

pub fn styled_combo_box<'a, T, Message>(
    state: &'a combo_box::State<T>,
    placeholder: &str,
    selection: Option<&T>,
    on_selected: impl Fn(T) -> Message + 'static,
) -> ComboBox<'a, T, Message>
where
    T: std::fmt::Display + Clone,
{
    combo_box(state, placeholder, selection, on_selected).input_style(|t, s| {
        let mut style = text_input::default(t, s);
        style.border.radius = Radius::default();
        style
    })
}

pub fn styled_pick_list<'a, T, L, V, Message>(
    options: L,
    selected: Option<V>,
    on_selected: impl Fn(T) -> Message + 'a,
) -> PickList<'a, T, L, V, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    L: Borrow<[T]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone,
{
    pick_list(options, selected, on_selected)
        .handle(pick_list::Handle::Dynamic {
            closed: pick_list::Icon {
                font: LUCIDE_FONT,
                code_point: const { char::from_u32(57459).unwrap() },
                size: None,
                line_height: 1.0.into(),
                shaping: Shaping::Advanced,
            },
            open: pick_list::Icon {
                font: LUCIDE_FONT,
                code_point: const { char::from_u32(57457).unwrap() },
                size: None,
                line_height: 1.0.into(),
                shaping: Shaping::Advanced,
            },
        })
        .style(|t, s| {
            let mut style = pick_list::default(t, s);
            style.border.radius = Radius::default();
            style.placeholder_color = t.extended_palette().background.weak.text;
            style
        })
}

pub fn styled_scrollable_with_direction<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    direction: impl Into<Direction>,
) -> Scrollable<'a, Message> {
    Scrollable::with_direction(content, direction).style(|t, s| {
        let mut style = scrollable::default(t, s);
        style.vertical_rail.border.radius = Radius::default();
        style.vertical_rail.scroller.border.radius = Radius::default();
        style.horizontal_rail.border.radius = Radius::default();
        style.horizontal_rail.scroller.border.radius = Radius::default();
        style
    })
}

pub fn styled_text_input<'a, Message>(placeholder: &str, value: &str) -> TextInput<'a, Message>
where
    Message: Clone,
{
    text_input(placeholder, value).style(|t, s| {
        let mut style = text_input::default(t, s);
        style.border.radius = Radius::default();
        style
    })
}
