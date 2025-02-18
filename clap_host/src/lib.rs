#![expect(missing_debug_implementations)]

use clack_host::prelude::*;
use gui::GuiExt;
use home::home_dir;
use host::Host;
use main_thread::MainThread;
use shared::Shared;
use std::{path::PathBuf, result::Result};
use walkdir::WalkDir;

mod clap_plugin_gui;
mod gui;
mod host;
mod host_audio_processor;
mod main_thread;
mod plugin_audio_processor;
mod shared;
mod timer;

pub use clack_host;
pub use clap_plugin_gui::ClapPluginGui;
pub use host::HostThreadMessage;
pub use host_audio_processor::HostAudioProcessor;
pub use main_thread::MainThreadMessage;
pub use plugin_audio_processor::PluginAudioProcessor;

#[must_use]
pub fn get_installed_plugins() -> Vec<PluginBundle> {
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
            unsafe { PluginBundle::load(path.path()) }.ok())
        .filter(|bundle| {
            bundle
                .get_plugin_factory()
                .is_some_and(|factory| factory.plugin_descriptors().next().is_some())
        })
        .collect()
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
pub fn init_gui(
    bundle: &PluginBundle,
    config: PluginAudioConfiguration,
) -> (ClapPluginGui, HostAudioProcessor, PluginAudioProcessor) {
    let (sender_host, receiver_plugin) = async_channel::bounded(16);
    let (sender_plugin, receiver_host) = async_channel::bounded(16);

    let factory = bundle.get_plugin_factory().unwrap();
    let plugin_descriptor = factory.plugin_descriptors().next().unwrap();
    let mut instance = PluginInstance::new(
        |()| Shared::new(sender_host.clone()),
        |_| MainThread::default(),
        bundle,
        plugin_descriptor.id().unwrap(),
        &HostInfo::new("", "", "", "").unwrap(),
    )
    .unwrap();

    let gui = instance
        .access_handler(|h: &MainThread<'_>| h.gui)
        .map(|gui| GuiExt::new(gui, &mut instance.plugin_handle()))
        .unwrap();

    let plugin_audio_processor = PluginAudioProcessor::new(
        instance
            .activate(|_, _| {}, config)
            .unwrap()
            .start_processing()
            .unwrap(),
        sender_plugin,
        receiver_plugin,
    );

    let host_audio_processor = HostAudioProcessor {
        sender: sender_host,
        receiver: receiver_host,
    };

    let gui = ClapPluginGui { instance, gui };

    (gui, host_audio_processor, plugin_audio_processor)
}
