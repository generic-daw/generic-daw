use iced::{
    Element, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Layout, Renderer as _, Widget,
        layout::{self, Limits, Node},
        renderer::{Quad, Style},
        widget::Tree,
    },
    mouse::Cursor,
};

#[derive(Clone, Copy, Debug)]
pub struct FileTreeIndicator {
    width: f32,
    height: f32,
    thickness: f32,
}

impl FileTreeIndicator {
    pub fn new(width: f32, height: f32, thickness: f32) -> Self {
        Self {
            width,
            height,
            thickness,
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for FileTreeIndicator {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.width), Length::Fixed(self.height))
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        layout::atomic(limits, self.width, self.height)
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        let offset = (self.width - self.thickness) * 0.5;

        let width = self.thickness.min(bounds.width - offset);

        if width <= 0.0 {
            return;
        }

        let height = offset.mul_add(-2.0, bounds.height.min(self.height));

        if height <= 0.0 {
            return;
        }

        let line = Quad {
            bounds: Rectangle::new(
                bounds.position() + Vector::new(offset, offset),
                Size::new(width, height),
            ),
            ..Default::default()
        };

        renderer.fill_quad(line, theme.extended_palette().primary.weak.color);
    }
}

impl<Message> From<FileTreeIndicator> for Element<'_, Message> {
    fn from(value: FileTreeIndicator) -> Self {
        Element::new(value)
    }
}
