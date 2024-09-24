use crate::generic_back::Arrangement;
use iced::{border::Radius, widget::container, Element, Theme};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub enum TimelineMessage {
    Tick,
}

pub struct Timeline {
    pub arrangement: Arc<Arrangement>,
}

impl Timeline {
    pub const fn new(arrangement: Arc<Arrangement>) -> Self {
        Self { arrangement }
    }

    #[expect(clippy::unused_self)]
    pub const fn update(&self, message: TimelineMessage) {
        match message {
            TimelineMessage::Tick => {}
        }
    }

    pub fn view(&self) -> Element<'_, TimelineMessage> {
        container(Element::new(self.arrangement.clone()))
            .style(|_| container::Style {
                border: iced::Border {
                    color: Theme::default().extended_palette().secondary.weak.color,
                    width: 1.0,
                    radius: Radius::new(0.0),
                },
                ..container::Style::default()
            })
            .into()
    }
}
