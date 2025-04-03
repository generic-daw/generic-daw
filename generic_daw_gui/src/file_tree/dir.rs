use super::file::File;
use crate::{
    components::{styled_button, styled_svg},
    daw::Message as DawMessage,
    icons::CHEVRON_RIGHT,
    widget::{Clipped, LINE_HEIGHT, shaping_of},
};
use iced::{
    Alignment, Element, Length, Radians, padding,
    widget::{
        column, container, mouse_area, row, rule, text,
        text::{Shaping, Wrapping},
        vertical_rule,
    },
};
use std::{f32::consts::FRAC_PI_2, path::Path, sync::Arc};

pub struct Dir {
    path: Arc<Path>,
    name: Arc<str>,
    shaping: Shaping,
    dirs: Option<Box<[Dir]>>,
    files: Option<Box<[File]>>,
    open: bool,
}

impl Dir {
    pub fn new(path: &Path) -> Self {
        let name = path.file_name().unwrap().to_str().unwrap();
        let shaping = shaping_of(name);

        Self {
            path: path.into(),
            name: name.into(),
            shaping,
            dirs: None,
            files: None,
            open: false,
        }
    }

    pub fn update(&mut self, path: &Path) {
        if &*self.path == path {
            self.open ^= true;

            if self.open {
                if self.dirs.is_none() {
                    self.dirs = Some(self.init_dirs());
                }

                if self.files.is_none() {
                    self.files = Some(self.init_files());
                }
            }
        } else if let Some(dirs) = self.dirs.as_mut() {
            dirs.iter_mut().for_each(|dir| dir.update(path));
        }
    }

    pub fn view(&self) -> (Element<'_, DawMessage>, f32) {
        let mut col = column!(mouse_area(
            styled_button(row![
                Clipped::new(
                    styled_svg(CHEVRON_RIGHT.clone())
                        .rotation(Radians(f32::from(u8::from(self.open)) * FRAC_PI_2))
                        .height(LINE_HEIGHT)
                ),
                container(
                    text(&*self.name)
                        .shaping(self.shaping)
                        .wrapping(Wrapping::None)
                )
                .clip(true)
            ])
            .width(Length::Fill)
            .padding(0)
            .on_press(DawMessage::FileTree(self.path.clone()))
        ));

        let mut height = 0.0;

        if self.open {
            let ch = column![column(
                self.dirs
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(Self::view)
                    .chain(self.files.as_ref().unwrap().iter().map(File::view))
                    .map(|(e, h)| {
                        height += h;
                        e
                    })
            ),];

            col = col.push(row![
                column![vertical_rule(1.0).style(|t| rule::Style {
                    width: 3,
                    ..rule::default(t)
                })]
                .padding(padding::top(LINE_HEIGHT / 2.0 - 1.5).bottom(LINE_HEIGHT / 2.0 - 1.5))
                .align_x(Alignment::Center)
                .width(LINE_HEIGHT)
                .height(height),
                ch
            ]);
        }

        (col.into(), height + LINE_HEIGHT)
    }

    fn init_files(&self) -> Box<[File]> {
        let Ok(files) = std::fs::read_dir(&self.path) else {
            return [].into();
        };

        let mut files = files
            .filter_map(Result::ok)
            .filter(|file| file.file_type().is_ok_and(|t| t.is_file()))
            .map(|file| {
                let mut name = file.file_name();
                name.make_ascii_lowercase();

                (file, name)
            })
            .collect::<Box<_>>();
        files.sort_unstable_by(|(_, aname), (_, bname)| aname.cmp(bname));
        files
            .iter()
            .map(|(entry, _)| File::new(Box::leak(entry.path().into_boxed_path())))
            .collect()
    }

    fn init_dirs(&self) -> Box<[Self]> {
        let Ok(dirs) = std::fs::read_dir(&self.path) else {
            return [].into();
        };

        let mut dirs = dirs
            .filter_map(Result::ok)
            .filter(|file| file.file_type().is_ok_and(|t| t.is_dir()))
            .map(|file| {
                let mut name = file.file_name();
                name.make_ascii_lowercase();

                (file, name)
            })
            .collect::<Box<_>>();
        dirs.sort_unstable_by(|(_, aname), (_, bname)| aname.cmp(bname));
        dirs.iter()
            .map(|(entry, _)| Self::new(Box::leak(entry.path().into_boxed_path())))
            .collect()
    }
}
