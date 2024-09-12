use std::cmp::{max_by, min_by};

use crate::generic_back::track::midi_track::MidiTrack;
use iced::{
    advanced::layout::{Layout, Node},
    widget::canvas::Frame,
    Size, Theme, Vector,
};

impl MidiTrack {
    pub fn draw(&self, frame: &mut Frame, theme: &Theme, layout: Layout) {
        let bounds = layout.bounds();

        self.clips.iter().for_each(|clip| {
            let left_bound = max_by(
                0.0,
                (clip
                    .get_global_start()
                    .in_interleaved_samples(&clip.arrangement.meter) as f32
                    - clip.arrangement.position.read().unwrap().x)
                    / clip.arrangement.scale.read().unwrap().x.exp2(),
                |a, b| a.partial_cmp(b).unwrap(),
            ) + bounds.x;

            let right_bound = min_by(
                bounds.width,
                (clip
                    .get_global_end()
                    .in_interleaved_samples(&clip.arrangement.meter) as f32
                    - clip.arrangement.position.read().unwrap().x)
                    / clip.arrangement.scale.read().unwrap().x.exp2(),
                |a, b| a.partial_cmp(b).unwrap(),
            ) + bounds.x;

            let node = Node::new(Size::new(right_bound - left_bound, bounds.height));
            let layout = Layout::with_offset(Vector::new(left_bound, bounds.y), &node);

            clip.draw(frame, theme, layout);
        });
    }
}
