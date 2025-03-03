use super::LINE_HEIGHT;
use iced::{
    Element, Event, Length, Rectangle, Renderer, Rotation, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Text, Widget,
        layout::{Limits, Node},
        mouse::{self, Click, Cursor},
        renderer::{Quad, Style},
        svg::{Handle, Renderer as _, Svg},
        text::{LineHeight, Renderer as _, Shaping, Wrapping},
        widget::{Tree, tree},
    },
    alignment::{Horizontal, Vertical},
    event::Status,
};
use std::path::Path;

#[derive(Default)]
struct State {
    last_click: Option<Click>,
    hovered: bool,
}

#[derive(Debug)]
pub struct FileTreeEntry<'a, Message> {
    path: &'a Path,
    name: String,
    svg: Handle,
    on_single_click: Option<fn(&'a Path) -> Message>,
    on_double_click: Option<fn(&'a Path) -> Message>,
    rotation: Rotation,
}

impl<'a, Message> FileTreeEntry<'a, Message> {
    pub fn new(
        path: &'a Path,
        svg: Handle,
        on_single_click: Option<fn(&'a Path) -> Message>,
        on_double_click: Option<fn(&'a Path) -> Message>,
    ) -> Self {
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();

        Self {
            path,
            name,
            svg,
            on_single_click,
            on_double_click,
            rotation: Rotation::default(),
        }
    }

    pub fn rotation(mut self, rotation: impl Into<Rotation>) -> Self {
        self.rotation = rotation.into();
        self
    }
}

impl<Message> Widget<Message, Theme, Renderer> for FileTreeEntry<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
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
        let bounds = layout.bounds();

        if !bounds.intersects(viewport) {
            return;
        };

        let background = Quad {
            bounds,
            ..Quad::default()
        };

        let state = tree.state.downcast_ref::<State>();

        let background_color = if state.hovered {
            theme.extended_palette().primary.base.color
        } else {
            theme.extended_palette().primary.strong.color
        };

        renderer.fill_quad(background, background_color);

        let icon = Svg::new(self.svg.clone())
            .color(theme.extended_palette().primary.strong.text)
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
            content: self.name.clone(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: renderer.default_size(),
            line_height: LineHeight::default(),
            font: renderer.default_font(),
            horizontal_alignment: Horizontal::Left,
            vertical_alignment: Vertical::Top,
            shaping: Shaping::Advanced,
            wrapping: Wrapping::None,
        };

        renderer.fill_text(
            name,
            bounds.position() + Vector::new(LINE_HEIGHT, 0.0),
            theme.extended_palette().primary.strong.text,
            bounds,
        );
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> Status {
        let state = tree.state.downcast_mut::<State>();

        let Some(pos) = cursor.position_in(layout.bounds()) else {
            state.hovered = false;
            return Status::Ignored;
        };

        state.hovered = true;

        if event == Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) {
            if let Some(on_single_click) = self.on_single_click {
                shell.publish(on_single_click(self.path));
            }

            if let Some(on_double_click) = self.on_double_click {
                let new_click = Click::new(pos, mouse::Button::Left, state.last_click);

                if matches!(new_click.kind(), mouse::click::Kind::Double) {
                    shell.publish(on_double_click(self.path));
                }

                state.last_click = Some(new_click);
            }

            return Status::Captured;
        }

        Status::Ignored
    }
}

impl<'a, Message> From<FileTreeEntry<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: FileTreeEntry<'a, Message>) -> Self {
        Element::new(value)
    }
}
