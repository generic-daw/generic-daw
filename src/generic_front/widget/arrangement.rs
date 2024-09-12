use crate::generic_back::arrangement::Arrangement;
use iced::{
    advanced::layout::{Layout, Node},
    widget::canvas::Frame,
    Size, Theme, Vector,
};

impl Arrangement {
    pub fn draw(&self, frame: &mut Frame, theme: &Theme, layout: Layout) {
        let bounds = layout.bounds();

        self.tracks
            .read()
            .unwrap()
            .iter()
            .enumerate()
            .for_each(|(i, track)| {
                let node = Node::new(Size::new(bounds.width, self.scale.read().unwrap().y));
                let layout = Layout::with_offset(
                    Vector::new(
                        bounds.x,
                        (i as f32).mul_add(self.scale.read().unwrap().y, bounds.y),
                    ),
                    &node,
                );
                track.draw(frame, theme, layout);
            });
    }
}
