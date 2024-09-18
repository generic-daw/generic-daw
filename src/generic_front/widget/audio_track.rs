use crate::generic_back::track::audio_track::AudioTrack;
use iced::{
    advanced::layout::{Layout, Node},
    Rectangle, Renderer, Size, Theme, Vector,
};

impl AudioTrack {
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        layout: Layout,
        clip_bounds: Rectangle,
    ) {
        let bounds = layout.bounds();

        self.clips.iter().for_each(|clip| {
            let first_pixel = (clip
                .get_global_start()
                .in_interleaved_samples(&clip.arrangement.meter)
                as f32
                - clip.arrangement.position.read().unwrap().x)
                / clip.arrangement.scale.read().unwrap().x.exp2()
                + bounds.x;

            let last_pixel = (clip
                .get_global_end()
                .in_interleaved_samples(&clip.arrangement.meter)
                as f32
                - clip.arrangement.position.read().unwrap().x)
                / clip.arrangement.scale.read().unwrap().x.exp2()
                + bounds.x;

            let node = Node::new(Size::new(last_pixel - first_pixel, bounds.height));
            let sublayout = Layout::with_offset(Vector::new(first_pixel, bounds.y), &node);

            let track_bounds = layout.bounds().intersection(&sublayout.bounds());
            if let Some(new_bounds) = track_bounds {
                let node = Node::new(new_bounds.size());
                let sublayout = Layout::with_offset(Vector::new(new_bounds.x, new_bounds.y), &node);
                clip.draw(renderer, theme, sublayout, clip_bounds);
            }
        });
    }
}
