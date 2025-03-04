use super::file::File;
use crate::{
    components::styled_button,
    daw::Message as DawMessage,
    widget::{FileTreeEntry, FileTreeIndicator, LINE_HEIGHT},
};
use iced::{
    Element, Radians, Rotation,
    widget::{column, mouse_area, row, svg},
};
use std::{f32::consts::FRAC_PI_2, path::Path, sync::LazyLock};

static DIR: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../../assets/material-symbols--chevron-right-rounded.svg"
    ))
});

pub struct Dir {
    path: Box<Path>,
    name: Box<str>,
    dirs: Option<Box<[Dir]>>,
    files: Option<Box<[File]>>,
    open: bool,
}

impl Dir {
    pub fn new(path: &Path) -> Self {
        let name = path.file_name().unwrap().to_str().unwrap().into();

        Self {
            path: path.into(),
            name,
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
            styled_button(FileTreeEntry::new(&self.name, DIR.clone()).rotation(
                Rotation::Floating(Radians(if self.open { FRAC_PI_2 } else { 0.0 }))
            ))
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

            col = col.push(row![FileTreeIndicator::new(LINE_HEIGHT, height, 2.0), ch]);
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
            .map(|(entry, _)| File::new(&entry.path()))
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
            .map(|(entry, _)| Self::new(&entry.path()))
            .collect()
    }
}
