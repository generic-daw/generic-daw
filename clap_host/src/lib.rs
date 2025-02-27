#![expect(missing_debug_implementations)]
#![expect(missing_copy_implementations)]

use audio_buffers::AudioBuffers;
use audio_ports_config::AudioPortsConfig;
use clack_host::prelude::*;
use generic_daw_utils::unique_id;
use host::Host;
use main_thread::MainThread;
use shared::Shared;
use std::{collections::BTreeMap, ffi::CString, path::PathBuf, result::Result};
use walkdir::WalkDir;

mod audio_buffers;
mod audio_ports_config;
mod audio_processor;
mod gui;
mod host;
mod main_thread;
mod note_buffers;
mod plugin_descriptor;
mod shared;
mod timer;

pub use async_channel::{Receiver, Sender};
pub use audio_processor::AudioProcessor;
pub use clack_host;
pub use gui::GuiExt;
pub use main_thread::GuiMessage;
pub use note_buffers::NoteBuffers;
pub use plugin_descriptor::PluginDescriptor;
pub use plugin_id::Id as PluginId;

unique_id!(plugin_id);

pub type AudioBuffer = Box<[Box<[f32]>]>;

#[must_use]
pub fn get_installed_plugins() -> BTreeMap<PluginDescriptor, PluginBundle> {
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
            unsafe { PluginBundle::load(path.path()) }.ok())
        .for_each(|bundle| {
            if let Some(factory) = bundle.get_plugin_factory() {
                factory
                    .plugin_descriptors()
                    .filter_map(|d| d.try_into().ok())
                    .for_each(|d| {
                        r.insert(d, bundle.clone());
                    });
            }
        });

    r
}

fn standard_clap_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    paths.push(
        #[expect(deprecated, reason = "rust#132515")]
        std::env::home_dir().unwrap().join(".clap"),
    );

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
        paths.push(
            #[expect(deprecated, reason = "rust#132515")]
            std::env::home_dir()
                .unwrap()
                .join("Library/Audio/Plug-Ins/CLAP"),
        );

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
    bundle: &PluginBundle,
    descriptor: &PluginDescriptor,
    sample_rate: f64,
    max_buffer_size: u32,
) -> (GuiExt, Receiver<GuiMessage>, AudioProcessor) {
    let (gui_sender, gui_receiver) = async_channel::unbounded();

    let mut instance = PluginInstance::new(
        |()| Shared::new(gui_sender),
        |shared| MainThread::new(shared),
        bundle,
        &CString::new(&*descriptor.id).unwrap(),
        &HostInfo::new("", "", "", "").unwrap(),
    )
    .unwrap();

    let input_config =
        AudioPortsConfig::from_ports(&mut instance.plugin_handle(), true).unwrap_or_default();
    let output_config =
        AudioPortsConfig::from_ports(&mut instance.plugin_handle(), false).unwrap_or_default();

    let channels =
        output_config.port_channel_counts[output_config.main_port_index].clamp(1, 2) as u32;
    let max_frames_count = max_buffer_size / channels;
    let config = PluginAudioConfiguration {
        sample_rate,
        min_frames_count: 1,
        max_frames_count,
    };

    let audio_buffers = AudioBuffers::new(config, input_config, output_config);
    let note_buffers = NoteBuffers::new(&mut instance.plugin_handle());

    let plugin_audio_processor = AudioProcessor::new(
        instance
            .activate(|_, _| {}, config)
            .unwrap()
            .start_processing()
            .unwrap(),
        audio_buffers,
        note_buffers,
    );

    let gui = GuiExt::new(instance.access_handler(|h| h.gui).unwrap(), instance);

    (gui, gui_receiver, plugin_audio_processor)
}
