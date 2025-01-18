use clack_host::prelude::*;
use gui::GuiExt;
use home::home_dir;
use host::{Host, HostThreadMessage};
use main_thread::{MainThread, MainThreadMessage};
use shared::Shared;
use std::{path::PathBuf, result::Result};
use walkdir::WalkDir;
use winit::raw_window_handle::RawWindowHandle;

mod clap_plugin_gui;
mod clap_plugin_gui_wrapper;
mod gui;
mod host;
mod host_audio_processor;
mod main_thread;
mod plugin_audio_processor;
mod shared;
mod timer;

pub use clack_host;
pub use clap_plugin_gui::ClapPluginGui;
pub use clap_plugin_gui_wrapper::ClapPluginGuiWrapper;
pub use host_audio_processor::HostAudioProcessor;
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

        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
    }

    #[cfg(target_os = "linux")]
    paths.push("/usr/lib/clap".into());

    if let Some(env_var) = std::env::var_os("CLAP_PATH") {
        paths.extend(std::env::split_paths(&env_var));
    }

    paths
}

#[must_use]
pub fn open_gui(
    bundle: &PluginBundle,
    config: PluginAudioConfiguration,
    window_handle: RawWindowHandle,
) -> (
    ClapPluginGuiWrapper,
    HostAudioProcessor,
    PluginAudioProcessor,
) {
    let (sender_host, receiver_plugin) = std::sync::mpsc::channel();
    let (sender_plugin, receiver_host) = std::sync::mpsc::channel();

    let factory = bundle.get_plugin_factory().unwrap();
    let plugin_descriptor = factory.plugin_descriptors().next().unwrap();
    let mut instance = PluginInstance::new(
        |()| Shared::new(sender_host.clone()),
        |shared| MainThread::new(shared),
        bundle,
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
        sender_plugin,
        receiver_plugin,
    );

    let host_audio_processor = HostAudioProcessor {
        sender: sender_host,
        receiver: receiver_host,
    };

    let mut gui = instance
        .access_handler(|h| h.gui)
        .map(|gui| GuiExt::new(gui, &mut instance.plugin_handle()))
        .unwrap();

    if gui.needs_floating().unwrap() {
        gui.open_floating(&mut instance.plugin_handle());
    } else {
        gui.open_embedded(&mut instance.plugin_handle(), window_handle);
    };

    let gui = ClapPluginGui::new(instance, gui);

    (
        ClapPluginGuiWrapper::new(gui),
        host_audio_processor,
        plugin_audio_processor,
    )
}
