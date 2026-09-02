use crate::{
	action::Action,
	components::virtualized,
	daw,
	stylefns::{
		container_with_radius, scrollable_style, scrollable_with_container, selectable_box,
		text_input_with_radius, weaker_bordered_box, weakest_bordered_box,
	},
	widget::LINE_HEIGHT,
};
use generic_daw_core::clap_host::{Category, Feature, PluginDescriptor, Subcategory};
use iced::{
	Element, Fill, FillPortion, Font, Shrink, Task,
	font::Weight,
	keyboard, padding,
	widget::{
		checkbox, column, container, mouse_area, operation::focus, row, scrollable, text,
		text_input,
	},
};
use std::{collections::HashSet, sync::Arc};
use unicode_segmentation::UnicodeSegmentation as _;
use utils::natural_cmp;

#[derive(Clone, Debug)]
pub enum Message {
	Query(Arc<str>),
	Category(Category),
	Subcategory(Subcategory),
	Hover(usize),
	HoverNext,
	HoverPrev,
	Select,
}

#[derive(Debug, Default)]
pub struct State {
	options: Vec<PluginDescriptor>,
	matchers: Vec<(Box<str>, Option<Box<str>>)>,
}

impl State {
	pub fn descriptors(&self) -> &[PluginDescriptor] {
		&self.options
	}
}

impl State {
	pub fn add(&mut self, descriptor: PluginDescriptor, plugin_picker: Option<&mut PluginPicker>) {
		if let Err(i) = self
			.options
			.binary_search_by(|d| natural_cmp(d.name.as_bytes(), descriptor.name.as_bytes()))
		{
			self.matchers.insert(
				i,
				(
					build_matcher(&descriptor.name),
					descriptor.vendor.as_deref().map(build_matcher),
				),
			);
			self.options.insert(i, descriptor);
			if let Some(plugin_picker) = plugin_picker {
				plugin_picker.search(self);
			}
		}
	}

	pub fn clear(&mut self, plugin_picker: Option<&mut PluginPicker>) {
		self.matchers.clear();
		self.options.clear();
		if let Some(plugin_picker) = plugin_picker {
			plugin_picker.search(self);
		}
	}
}

#[derive(Debug)]
pub struct PluginPicker {
	query: Arc<str>,
	categories: HashSet<Category>,
	subcategories: HashSet<Subcategory>,
	filtered_options: Vec<usize>,
	hovered_option: usize,
}

impl PluginPicker {
	pub fn create(state: &State) -> (Self, Task<Message>) {
		(
			Self {
				query: Arc::default(),
				categories: HashSet::with_capacity(Category::VARIANTS.len()),
				subcategories: HashSet::with_capacity(Subcategory::VARIANTS.len()),
				filtered_options: (0..state.options.len()).collect(),
				hovered_option: 0,
			},
			focus("plugin picker input"),
		)
	}

	pub fn update(&mut self, message: Message, state: &State) -> Action<daw::Instruction, Message> {
		match message {
			Message::Query(query) => {
				self.query = query;
				self.search(state);
			}
			Message::Category(category) => {
				if !self.categories.remove(&category) {
					self.categories.insert(category);
				}
				self.search(state);
			}
			Message::Subcategory(subcategory) => {
				if !self.subcategories.remove(&subcategory) {
					self.subcategories.insert(subcategory);
				}
				self.search(state);
			}
			Message::Hover(i) => self.hovered_option = i,
			Message::HoverNext => self.hovered_option += 1,
			Message::HoverPrev => self.hovered_option = self.hovered_option().saturating_sub(1),
			Message::Select => {
				if !self.filtered_options.is_empty() {
					return Action::batch([
						Action::instruction(daw::Instruction::PluginLoad(
							state.options[self.filtered_options[self.hovered_option()]].clone(),
						)),
						Action::instruction(daw::Instruction::Message(
							daw::Message::TogglePluginPicker,
						)),
					]);
				}
			}
		}

		Action::none()
	}

	pub fn view<'a>(&'a self, state: &'a State) -> Element<'a, Message> {
		scrollable(
			column![
				text_input("Add Plugin", &*self.query)
					.id("plugin picker input")
					.on_input(|query| Message::Query(query.into()))
					.style(text_input_with_radius(text_input::default, 5)),
				row![
					column![
						column![
							text("Category").font(Font::DEFAULT.weight(Weight::Bold)),
							container(column(Category::VARIANTS.iter().map(|category| {
								checkbox(
									self.categories.is_empty()
										|| self.categories.contains(category),
								)
								.label(category.to_string())
								.on_toggle(|_| Message::Category(*category))
								.width(Fill)
								.into()
							})))
							.padding(padding::horizontal(5).vertical(2.6))
							.style(container_with_radius(weakest_bordered_box, 5))
						]
						.spacing(5),
						column![
							text("Subcategory").font(Font::DEFAULT.weight(Weight::Bold)),
							container(column(Subcategory::VARIANTS.iter().map(|subcategory| {
								checkbox(
									self.subcategories.is_empty()
										|| self.subcategories.contains(subcategory),
								)
								.label(subcategory.to_string())
								.on_toggle(|_| Message::Subcategory(*subcategory))
								.width(Fill)
								.into()
							})))
							.padding(padding::horizontal(5).vertical(2.6))
							.style(container_with_radius(weakest_bordered_box, 5))
						]
						.spacing(5),
					]
					.spacing(10),
					scrollable(
						column(
							self.filtered_options
								.iter()
								.map(|&i| &state.options[i])
								.enumerate()
								.map(|(i, descriptor)| virtualized(move || {
									mouse_area(
										container(
											row![
												text(&*descriptor.name)
													.wrapping(text::Wrapping::None)
													.ellipsis(text::Ellipsis::End),
												text(
													descriptor
														.version
														.as_deref()
														.unwrap_or_default(),
												)
												.wrapping(text::Wrapping::None)
												.ellipsis(text::Ellipsis::End)
												.style(text::secondary)
												.width(Fill),
												text(
													descriptor
														.vendor
														.as_deref()
														.unwrap_or_default(),
												)
												.wrapping(text::Wrapping::None)
												.ellipsis(text::Ellipsis::Start)
												.style(text::secondary),
											]
											.spacing(5)
											.padding(padding::horizontal(3)),
										)
										.padding(1)
										.height(LINE_HEIGHT + 2.0)
										.style(container_with_radius(
											selectable_box(
												container::transparent,
												i == self.hovered_option(),
											),
											5,
										)),
									)
									.on_enter(Message::Hover(i))
									.on_press(Message::Select)
									.into()
								}))
						)
						.width(Fill)
						.padding(5)
					)
					.width(FillPortion(3))
					.height(Fill)
					.direction(scrollable::Direction::Vertical(
						scrollable::Scrollbar::hidden(),
					))
					.style(scrollable_with_container(
						scrollable::default,
						container_with_radius(weaker_bordered_box, 5)
					))
				]
				.height(Shrink)
				.spacing(10)
			]
			.spacing(10)
			.padding(10),
		)
		.width(720)
		.spacing(5)
		.style(scrollable_with_container(
			scrollable_style,
			container_with_radius(weakest_bordered_box, 10),
		))
		.into()
	}

	pub fn keybinds(
		key: &keyboard::Key,
		modifiers: keyboard::Modifiers,
		repeat: bool,
	) -> Option<daw::Message> {
		match (
			modifiers.command(),
			modifiers.shift(),
			modifiers.alt(),
			repeat,
		) {
			(false, false, false, repeat) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::Escape) if !repeat => {
					Some(daw::Message::TogglePluginPicker)
				}
				keyboard::Key::Named(keyboard::key::Named::Enter) if !repeat => {
					Some(daw::Message::PluginPicker(Message::Select))
				}
				keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
					Some(daw::Message::PluginPicker(Message::HoverPrev))
				}
				keyboard::Key::Named(
					keyboard::key::Named::ArrowDown | keyboard::key::Named::Tab,
				) => Some(daw::Message::PluginPicker(Message::HoverNext)),
				_ => None,
			},
			(false, true, false, _) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::Tab) => {
					Some(daw::Message::PluginPicker(Message::HoverPrev))
				}
				_ => None,
			},
			_ => None,
		}
	}

	fn search(&mut self, state: &State) {
		let query = self
			.query
			.unicode_words()
			.map(str::to_lowercase)
			.collect::<Vec<_>>();

		self.filtered_options.clear();
		self.filtered_options.extend(
			state
				.options
				.iter()
				.zip(&state.matchers)
				.enumerate()
				.filter(|(_, (descriptor, _))| {
					self.categories.is_empty()
						|| self.categories.iter().any(|&category| {
							descriptor.features.contains(&Feature::Category(category))
						})
				})
				.filter(|(_, (descriptor, _))| {
					self.subcategories.is_empty()
						|| self.subcategories.iter().any(|&subcategory| {
							descriptor
								.features
								.contains(&Feature::Subcategory(subcategory))
						})
				})
				.filter(|(_, (_, (name, vendor)))| {
					query.iter().all(|word| {
						name.contains(word)
							|| vendor
								.as_deref()
								.is_some_and(|vendor| vendor.contains(word))
					})
				})
				.map(|(i, _)| i),
		);
	}

	fn hovered_option(&self) -> usize {
		self.hovered_option
			.min(self.filtered_options.len().saturating_sub(1))
	}
}

fn build_matcher(keyword: &str) -> Box<str> {
	keyword
		.unicode_words()
		.flat_map(str::chars)
		.flat_map(char::to_lowercase)
		.collect()
}
