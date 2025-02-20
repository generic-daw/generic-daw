#![expect(missing_debug_implementations)]

use clack_host::prelude::*;
use generic_daw_utils::unique_id;
use home::home_dir;
use host::Host;
use main_thread::MainThread;
use shared::Shared;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    result::Result,
};
use walkdir::WalkDir;

mod gui;
mod host;
mod host_audio_processor;
mod main_thread;
mod plugin_audio_processor;
mod shared;
mod timer;

pub use async_channel::{Receiver, Sender};
pub use clack_host;
pub use gui::GuiExt;
pub use host_audio_processor::HostAudioProcessor;
pub use main_thread::GuiMessage;
pub use plugin_audio_processor::PluginAudioProcessor;
pub use plugin_id::Id as PluginId;

unique_id!(plugin_id);

#[must_use]
pub fn get_installed_plugins() -> BTreeMap<String, PathBuf> {
    let mut r = BTreeMap::new();

    standard_clap_paths()
        .iter()
        .flat_map(|path| {
            WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|dir_entry| dir_entry.file_type().is_file())
                .filter(|dir_entry| {
                    dir_entry
                        .path()
                        .extension()
                        .is_some_and(|ext| ext == "clap")
                })
        })
        .filter_map(|path|
            // SAFETY:
            // loading an external library object file is inherently unsafe
            Some((path.path().to_owned(), unsafe { PluginBundle::load(path.path()) }.ok()?)))
        .for_each(|(path, bundle)| {
            if let Some(factory) = bundle.get_plugin_factory() {
                factory
                    .plugin_descriptors()
                    .filter_map(|d| d.name()?.to_str().ok())
                    .for_each(|d| {
                        r.insert(d.to_owned(), path.clone());
                    });
            }
        });

    r
}

fn standard_clap_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    paths.push(home_dir().unwrap().join(".clap"));

    #[cfg(target_os = "windows")]
    {
        if let Some(val) = std::env::var_os("CommonProgramFiles") {
            paths.push(PathBuf::from(val).join("CLAP"));
        }

        use etcetera::BaseStrategy as _;

        paths.push(
            etcetera::choose_base_strategy()
                .unwrap()
                .config_dir()
                .join("Programs\\Common\\CLAP"),
        );
    }

    #[cfg(target_os = "macos")]
    {
        paths.push(home_dir().unwrap().join("Library/Audio/Plug-Ins/CLAP"));

        paths.push("/Library/Audio/Plug-Ins/CLAP".into());
    }

    #[cfg(target_os = "linux")]
    paths.push("/usr/lib/clap".into());

    if let Some(env_var) = std::env::var_os("CLAP_PATH") {
        paths.extend(std::env::split_paths(&env_var));
    }

    paths
}

#[must_use]
pub fn init(
    path: &Path,
    name: &str,
    config: PluginAudioConfiguration,
) -> (
    GuiExt,
    Receiver<GuiMessage>,
    HostAudioProcessor,
    PluginAudioProcessor,
) {
    let (gui_sender, gui_receiver) = async_channel::bounded(16);
    let (plugin_sender, plugin_receiver) = async_channel::bounded(16);
    let (audio_sender, audio_receiver) = async_channel::bounded(16);

    // SAFETY:
    // loading an external library object file is inherently unsafe
    let bundle = unsafe { PluginBundle::load(path) }.unwrap();

    let factory = bundle.get_plugin_factory().unwrap();
    let plugin_descriptor = factory
        .plugin_descriptors()
        .find(|d| d.name().and_then(|n| n.to_str().ok()) == Some(name))
        .unwrap();
    let mut instance = PluginInstance::new(
        |()| Shared::new(gui_sender),
        |_| MainThread::default(),
        &bundle,
        plugin_descriptor.id().unwrap(),
        &HostInfo::new("", "", "", "").unwrap(),
    )
    .unwrap();

    let plugin_audio_processor = PluginAudioProcessor::new(
        instance
            .activate(|_, _| {}, config)
            .unwrap()
            .start_processing()
            .unwrap(),
        plugin_sender,
        audio_receiver,
    );

    let gui = GuiExt::new(
        instance.access_handler(|h: &MainThread| h.gui).unwrap(),
        instance,
    );

    (
        gui,
        gui_receiver,
        HostAudioProcessor {
            sender: audio_sender,
            receiver: plugin_receiver,
        },
        plugin_audio_processor,
    )
}
