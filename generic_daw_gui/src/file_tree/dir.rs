use super::{Action, Message, file::File};
use crate::{
    components::styled_button,
    icons::CHEVRON_RIGHT,
    widget::{Clipped, LINE_HEIGHT, shaping_of},
};
use iced::{
    Alignment, Element, Fill, Radians, Shrink, Task, padding,
    widget::{
        column, container, row, rule, svg, text,
        text::{Shaping, Wrapping},
        vertical_rule,
    },
};
use smol::stream::StreamExt as _;
use std::{f32::consts::FRAC_PI_2, path::Path, sync::Arc};

#[derive(Clone, Debug)]
pub struct Dir {
    path: Arc<Path>,
    name: Arc<str>,
    shaping: Shaping,
    children: LoadStatus,
    open: bool,
}

#[derive(Clone, Debug)]
enum LoadStatus {
    Unloaded,
    Loading,
    Loaded {
        dirs: Box<[Dir]>,
        files: Box<[File]>,
    },
}

impl Dir {
    pub fn new(path: &Path) -> Self {
        let name = path.file_name().unwrap().to_str().unwrap();
        let shaping = shaping_of(name);

        Self {
            path: path.into(),
            name: name.into(),
            shaping,
            children: LoadStatus::Unloaded,
            open: false,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn update(&mut self, path: &Path, action: Action) -> Task<Message> {
        if *path == *self.path {
            match action {
                Action::DirOpened(dirs, files) => {
                    self.children = LoadStatus::Loaded { dirs, files };
                }
                Action::DirToggleOpen => {
                    self.open ^= true;

                    if matches!(self.children, LoadStatus::Unloaded) {
                        self.children = LoadStatus::Loading;
                        let path = self.path.clone();

                        return Task::perform(Self::load(self.path.clone()), |(dirs, files)| {
                            Message::Action(path, Action::DirOpened(dirs, files))
                        });
                    }
                }
            }
        } else if let LoadStatus::Loaded { dirs, .. } = &mut self.children {
            return dirs
                .iter_mut()
                .find(|dir| path.starts_with(dir.path()))
                .unwrap()
                .update(path, action);
        }

        Task::none()
    }

    pub fn view(&self) -> (Element<'_, Message>, f32) {
        let mut col = column!(
            styled_button(row![
                Clipped::new(
                    svg(CHEVRON_RIGHT.clone())
                        .rotation(Radians(f32::from(u8::from(self.open)) * FRAC_PI_2))
                        .width(Shrink)
                        .height(LINE_HEIGHT)
                ),
                container(
                    text(&*self.name)
                        .shaping(self.shaping)
                        .wrapping(Wrapping::None)
                )
                .clip(true)
            ])
            .width(Fill)
            .padding(0)
            .on_press(Message::Action(self.path.clone(), Action::DirToggleOpen))
        );

        let mut height = 0.0;

        if self.open {
            if let LoadStatus::Loaded { dirs, files } = &self.children {
                let ch = column![column(
                    dirs.iter()
                        .map(Self::view)
                        .chain(files.iter().map(File::view))
                        .map(|(e, h)| {
                            height += h;
                            e
                        })
                ),];

                if height != 0.0 {
                    col = col.push(row![
                        column![vertical_rule(1.0).style(|t| rule::Style {
                            width: 3,
                            ..rule::default(t)
                        })]
                        .padding(
                            padding::top(LINE_HEIGHT / 2.0 - 1.5).bottom(LINE_HEIGHT / 2.0 - 1.5)
                        )
                        .align_x(Alignment::Center)
                        .width(LINE_HEIGHT)
                        .height(height),
                        ch
                    ]);
                }
            }
        }

        (col.into(), height + LINE_HEIGHT)
    }

    async fn load(path: Arc<Path>) -> (Box<[Self]>, Box<[File]>) {
        let Ok(mut entry) = smol::fs::read_dir(path).await else {
            return ([].into(), [].into());
        };

        let mut files = Vec::new();
        let mut dirs = Vec::new();

        while let Some(entry) = entry.next().await {
            let Ok(entry) = entry else {
                continue;
            };

            let Ok(ty) = entry.file_type().await else {
                continue;
            };

            let mut name = entry.file_name();
            name.make_ascii_lowercase();

            if ty.is_file() {
                files.push((entry, name));
            } else if ty.is_dir() {
                dirs.push((entry, name));
            }
        }

        files.sort_unstable_by(|(_, aname), (_, bname)| aname.cmp(bname));
        dirs.sort_unstable_by(|(_, aname), (_, bname)| aname.cmp(bname));

        let files = files
            .into_iter()
            .map(|(entry, _)| File::new(&entry.path()))
            .collect();
        let dirs = dirs
            .into_iter()
            .map(|(entry, _)| Self::new(&entry.path()))
            .collect();

        (dirs, files)
    }
}
