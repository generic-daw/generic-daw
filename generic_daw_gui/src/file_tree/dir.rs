use super::{Action, Message, file::File};
use crate::{
    components::styled_button,
    icons::{chevron_down, chevron_right},
    widget::{LINE_HEIGHT, shaping_of},
};
use generic_daw_utils::unique_id;
use iced::{
    Alignment, Element, Fill, Task, padding,
    widget::{
        column, container, row, rule, text,
        text::{Shaping, Wrapping},
        vertical_rule,
    },
};
use smol::stream::StreamExt as _;
use std::{path::Path, sync::Arc};

unique_id!(dir_entry);

pub use dir_entry::Id as DirId;

#[derive(Clone, Debug)]
pub struct Dir {
    id: DirId,
    name: Arc<str>,
    path: Arc<Path>,
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
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let name = path.file_name().unwrap().to_str().unwrap();
        let shaping = shaping_of(name);

        Self {
            id: DirId::unique(),
            name: name.into(),
            path: path.into(),
            shaping,
            children: LoadStatus::Unloaded,
            open: false,
        }
    }

    pub fn update(&mut self, id: DirId, action: &Action) -> Option<Task<Message>> {
        if id == self.id {
            Some(match action {
                Action::DirOpened(dirs, files) => {
                    self.children = LoadStatus::Loaded {
                        dirs: dirs.clone(),
                        files: files.clone(),
                    };

                    Task::none()
                }
                Action::DirToggleOpen => {
                    self.open ^= true;

                    if matches!(self.children, LoadStatus::Unloaded) {
                        let path = self.path.clone();
                        let id = self.id;
                        self.children = LoadStatus::Loading;

                        Task::perform(Self::load(path), move |(dirs, files)| {
                            Message::Action(id, Action::DirOpened(dirs, files))
                        })
                    } else {
                        Task::none()
                    }
                }
            })
        } else if let LoadStatus::Loaded { dirs, .. } = &mut self.children {
            dirs.iter_mut().find_map(|dir| dir.update(id, action))
        } else {
            None
        }
    }

    pub fn view(&self) -> (Element<'_, Message>, f32) {
        let mut col = column!(
            styled_button(row![
                container(if self.open {
                    chevron_down()
                } else {
                    chevron_right()
                })
                .clip(true),
                container(
                    text(&*self.name)
                        .shaping(self.shaping)
                        .wrapping(Wrapping::None)
                )
                .clip(true)
            ])
            .width(Fill)
            .padding(0)
            .on_press(Message::Action(self.id, Action::DirToggleOpen))
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
                )];

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

    async fn load(path: impl AsRef<Path>) -> (Box<[Self]>, Box<[File]>) {
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
            .map(|(entry, _)| File::new(entry.path()))
            .collect();
        let dirs = dirs
            .into_iter()
            .map(|(entry, _)| Self::new(entry.path()))
            .collect();

        (dirs, files)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
