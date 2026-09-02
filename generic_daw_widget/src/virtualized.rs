use iced_widget::{
	Renderer, Theme, component,
	core::{Element, Length, Size},
	sensor, space,
};
use std::cell::Cell;

pub struct Virtualized<'a, Message> {
	f: Box<dyn Fn() -> Element<'a, Message, Theme, Renderer> + 'a>,
	cache: Cell<Option<Element<'a, Message, Theme, Renderer>>>,
	size: Size<Length>,
}

impl<'a, Message> Virtualized<'a, Message> {
	pub fn new(f: impl Fn() -> Element<'a, Message, Theme, Renderer> + 'a) -> Self {
		let cache = f();
		Self {
			size: cache.as_widget().size(),
			cache: Cell::new(Some(cache)),
			f: Box::from(f),
		}
	}
}

impl<'a, Message: 'static> component::Component<'a, Message> for Virtualized<'a, Message> {
	type State = bool;
	type Event = Option<Message>;

	fn update(
		&mut self,
		state: &mut Self::State,
		event: Self::Event,
		_renderer: &Renderer,
	) -> Option<Message> {
		*state ^= event.is_none();
		event
	}

	fn view(&self, state: &Self::State) -> Element<'a, Self::Event, Theme, Renderer> {
		sensor(if *state {
			self.cache.take().unwrap_or_else(&self.f).map(Some)
		} else {
			space()
				.width(self.size.width)
				.height(self.size.height)
				.into()
		})
		.on_show(|_| None)
		.on_hide(None)
		.into()
	}
}

impl<'a, Message: 'static> From<Virtualized<'a, Message>>
	for Element<'a, Message, Theme, Renderer>
{
	fn from(value: Virtualized<'a, Message>) -> Self {
		component(value)
	}
}
