use crate::stylefns::{button_with_base, svg_with_enabled};
use iced::{
    Alignment, Element, Length,
    border::Radius,
    widget::{
        Button, ComboBox, PickList, Scrollable, Space, Svg, button, combo_box, container,
        pick_list,
        scrollable::{self, Direction},
        svg, text, text_input,
    },
};
use std::borrow::Borrow;

pub fn empty_widget() -> Space {
    Space::new(Length::Shrink, Length::Shrink)
}

pub fn char_button<'a, Message>(t: char) -> Button<'a, Message>
where
    Message: Clone + 'a,
{
    button(
        container(text(t).size(13.0).line_height(1.0))
            .width(13.0)
            .align_x(Alignment::Center),
    )
    .padding(0.0)
}

pub fn styled_button<'a, Message>(content: impl Into<Element<'a, Message>>) -> Button<'a, Message> {
    button(content).style(|t, s| button_with_base(t, s, button::primary))
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
    pick_list(options, selected, on_selected).style(|t, s| {
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

pub fn styled_svg<'a>(handle: impl Into<svg::Handle>) -> Svg<'a> {
    svg(handle)
        .style(|t, s| svg_with_enabled(t, s, true))
        .width(Length::Shrink)
        .height(Length::Shrink)
}
