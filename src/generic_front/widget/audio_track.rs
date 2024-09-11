use crate::{
    generic_back::{track::audio_track::AudioTrack, track_clip::audio_clip::AudioClip},
    generic_front::timeline::Message,
};
use iced::{
    advanced::{
        graphics::geometry::{Frame, Renderer as _},
        layout::{self, Layout, Node},
        renderer,
        widget::{self, Widget},
    },
    mouse,
    widget::canvas::Path,
    Length, Point, Rectangle, Renderer, Size, Theme, Vector,
};
use std::{
    cmp::{max_by, min_by},
    sync::{Arc, RwLock},
};

impl Widget<Message, Theme, Renderer> for Arc<RwLock<AudioTrack>> {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(
        &self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::new(limits.max().width, limits.max().height))
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let mut frame = Frame::new(renderer, bounds.size());
        let path = Path::new(|path| {
            path.line_to(Point::new(0.0, bounds.height - 2.0));
            path.line_to(Point::new(bounds.width, bounds.height - 2.0));
        });

        frame.with_clip(bounds, |frame| {
            frame.stroke(
                &path,
                iced::widget::canvas::Stroke::default()
                    .with_color(theme.extended_palette().secondary.weak.color),
            );
        });

        self.read().unwrap().clips.iter().for_each(|clip| {
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

            <AudioClip as Widget<Message, Theme, Renderer>>::draw(
                clip, tree, renderer, theme, style, layout, cursor, viewport,
            );
        });

        renderer.draw_geometry(frame.into_geometry());
    }
}
