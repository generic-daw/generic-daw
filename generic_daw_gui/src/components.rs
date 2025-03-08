use core::f32;
use iced::{
    Element, Length, Theme,
    border::{self, Radius},
    widget::{
        Button, Container, PickList, Scrollable, Svg, button, container, pick_list,
        scrollable::{self, Direction},
        svg,
    },
};
use std::borrow::Borrow;

pub fn styled_button<'a, Message>(content: impl Into<Element<'a, Message>>) -> Button<'a, Message> {
    button(content).style(|t, s| {
        let mut style = button::primary(t, s);
        style.border.radius = Radius::default();
        style
    })
}

pub fn round_danger_button<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> Button<'a, Message> {
    button(content).style(|t, s| {
        let mut style = button::danger(t, s);
        style.border.radius = Radius::new(f32::INFINITY);
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
        .style(|t: &Theme, _| svg::Style {
            color: Some(t.extended_palette().primary.base.text),
        })
        .width(Length::Shrink)
        .height(Length::Shrink)
}

pub fn styled_container<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> Container<'a, Message> {
    container(content).style(|t: &Theme| {
        let mut style = container::transparent(t);
        style.background = Some(t.extended_palette().background.weak.color.into());
        style.border = border::width(1.0).color(t.extended_palette().background.strong.color);
        style
    })
}
