#![expect(missing_debug_implementations)]

use audio_processor::AudioProcessor;
use clack_host::prelude::*;
use gui::GuiExt;
use home::home_dir;
use host::{Host, HostThreadMessage};
use main_thread::{MainThread, MainThreadMessage};
use shared::Shared;
use std::{path::PathBuf, result::Result};
use walkdir::WalkDir;
use winit::raw_window_handle::RawWindowHandle;

pub use clap_plugin::ClapPlugin;
pub use clap_plugin_wrapper::ClapPluginWrapper;

mod audio_processor;
mod clap_plugin;
mod clap_plugin_wrapper;
mod gui;
mod host;
mod main_thread;
mod shared;
mod timer;

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
        .filter_map(|path| unsafe { PluginBundle::load(path.path()) }.ok())
        .filter(|bundle| {
            bundle
                .get_plugin_factory()
                .is_some_and(|factory| factory.plugin_descriptors().next().is_some())
        })
        .collect()
}

fn standard_clap_paths() -> Vec<PathBuf> {
    let mut paths = vec![];

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

pub fn open_gui(
    bundle: &PluginBundle,
    config: PluginAudioConfiguration,
    window_handle: RawWindowHandle,
) -> ClapPlugin {
    let (sender_host, _receiver_plugin) = std::sync::mpsc::channel();
    let (_sender_plugin, receiver_host) = std::sync::mpsc::channel();

    let factory = bundle.get_plugin_factory().unwrap();
    let plugin_descriptor = factory.plugin_descriptors().next().unwrap();
    let mut instance = PluginInstance::<Host>::new(
        |()| Shared::new(sender_host.clone()),
        |shared| MainThread::new(shared),
        bundle,
        plugin_descriptor.id().unwrap(),
        &HostInfo::new("", "", "", "").unwrap(),
    )
    .unwrap();

    let _audio_processor = AudioProcessor::new(
        instance
            .activate(|_, _| {}, config)
            .unwrap()
            .start_processing()
            .unwrap(),
    );

    let mut gui = instance
        .access_handler(|h| h.gui)
        .map(|gui| GuiExt::new(gui, &mut instance.plugin_handle()))
        .unwrap();

    if gui.needs_floating().unwrap() {
        gui.open_floating(&mut instance.plugin_handle());
    } else {
        gui.open_embedded(&mut instance.plugin_handle(), window_handle);
    };

    ClapPlugin::new(instance, gui, sender_host, receiver_host)
}
