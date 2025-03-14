use super::{LINE_HEIGHT, shaping_of};
use iced::{
    Element, Length, Rectangle, Renderer, Rotation, Size, Theme, Vector,
    advanced::{
        Layout, Renderer as _, Text, Widget,
        layout::{Limits, Node},
        mouse::Cursor,
        renderer::Style,
        svg::{Handle, Renderer as _, Svg},
        text::{LineHeight, Renderer as _, Shaping, Wrapping},
        widget::{Tree, tree},
    },
    alignment::Vertical,
    widget::text::Alignment,
};

struct State {
    shaping: Shaping,
}

impl State {
    fn new(text: &str) -> Self {
        Self {
            shaping: shaping_of(text),
        }
    }
}

#[derive(Debug)]
pub struct FileTreeEntry<'a> {
    name: &'a str,
    svg: Handle,
    rotation: Rotation,
}

impl<'a> FileTreeEntry<'a> {
    pub fn new(name: &'a str, svg: Handle) -> Self {
        Self {
            name,
            svg,
            rotation: Rotation::default(),
        }
    }

    pub fn rotation(mut self, rotation: impl Into<Rotation>) -> Self {
        self.rotation = rotation.into();
        self
    }
}

impl<Message> Widget<Message, Theme, Renderer> for FileTreeEntry<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.name))
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        Node::new(Size::new(limits.max().width, LINE_HEIGHT))
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();

        if !bounds.intersects(viewport) {
            return;
        }

        let icon = Svg::new(self.svg.clone())
            .color(theme.extended_palette().primary.base.text)
            .rotation(self.rotation.radians());

        if bounds.width < LINE_HEIGHT || bounds.height < LINE_HEIGHT {
            renderer.start_layer(bounds);
        }

        renderer.draw_svg(
            icon,
            Rectangle::new(bounds.position(), Size::new(LINE_HEIGHT, LINE_HEIGHT)),
        );

        if bounds.width < LINE_HEIGHT || bounds.height < LINE_HEIGHT {
            renderer.end_layer();
        }

        let name = Text {
            content: self.name.to_owned(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: renderer.default_size(),
            line_height: LineHeight::default(),
            font: renderer.default_font(),
            align_x: Alignment::Left,
            align_y: Vertical::Top,
            shaping: state.shaping,
            wrapping: Wrapping::None,
        };

        renderer.fill_text(
            name,
            bounds.position() + Vector::new(LINE_HEIGHT, 0.0),
            theme.extended_palette().primary.base.text,
            bounds,
        );
    }
}

impl<'a, Message> From<FileTreeEntry<'a>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: FileTreeEntry<'a>) -> Self {
        Element::new(value)
    }
}
