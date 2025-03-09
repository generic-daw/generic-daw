use crate::stylefns::button_with_enabled;
use iced::{
    Element, Length, Theme,
    border::Radius,
    widget::{
        Button, PickList, Scrollable, Svg, button, pick_list,
        scrollable::{self, Direction},
        svg,
    },
};
use std::borrow::Borrow;

pub fn styled_button<'a, Message>(content: impl Into<Element<'a, Message>>) -> Button<'a, Message> {
    button(content).style(|t, s| button_with_enabled(t, s, true))
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
        .style(|t: &Theme, _| svg::Style {
            color: Some(t.extended_palette().primary.base.text),
        })
        .width(Length::Shrink)
        .height(Length::Shrink)
}
