use iced_widget::{Renderer, Theme, component, core::Element};

pub struct Stateful<'a, State, Event, Message> {
	update: Box<dyn Fn(&mut State, Event) -> Option<Message> + 'a>,
	view: Box<dyn Fn(&State) -> Element<'a, Event, Theme, Renderer> + 'a>,
}

impl<'a, State, Event, Message> Stateful<'a, State, Event, Message> {
	pub fn new(
		update: impl Fn(&mut State, Event) -> Option<Message> + 'a,
		view: impl Fn(&State) -> Element<'a, Event, Theme, Renderer> + 'a,
	) -> Self {
		Self {
			update: Box::new(update),
			view: Box::new(view),
		}
	}
}

impl<'a, State: Default + 'static, Event: 'static, Message> component::Component<'a, Message>
	for Stateful<'a, State, Event, Message>
{
	type State = State;
	type Event = Event;

	fn update(
		&mut self,
		state: &mut Self::State,
		event: Self::Event,
		_renderer: &Renderer,
	) -> Option<Message> {
		(self.update)(state, event)
	}

	fn view(&self, state: &Self::State) -> Element<'a, Self::Event, Theme, Renderer> {
		(self.view)(state)
	}
}

impl<'a, State: Default + 'static, Event: 'static, Message: 'a>
	From<Stateful<'a, State, Event, Message>> for Element<'a, Message, Theme, Renderer>
{
	fn from(value: Stateful<'a, State, Event, Message>) -> Self {
		component(value)
	}
}
