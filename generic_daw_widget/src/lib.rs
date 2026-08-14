#![expect(missing_debug_implementations)]

use iced_widget::core::Element;
use std::cell::LazyCell;

pub mod context_menu;
pub mod drag_handle;
pub mod knob;
pub mod menu;
pub mod menu_overlay;
pub mod peak_meter;
pub mod select_area;

type LazyElement<'a, Message, Theme, Renderer> = LazyCell<
	Element<'a, Message, Theme, Renderer>,
	Box<dyn Fn() -> Element<'a, Message, Theme, Renderer> + 'a>,
>;
