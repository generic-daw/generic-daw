use iced::{
    Element,
    border::Radius,
    widget::{Button, PickList, button, pick_list},
};
use std::borrow::Borrow;

pub fn styled_button<'a, Message>(content: impl Into<Element<'a, Message>>) -> Button<'a, Message> {
    button(content).style(|t, s| {
        let mut style = button::primary(t, s);
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
