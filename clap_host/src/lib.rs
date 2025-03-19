use async_channel::Receiver;
use audio_buffers::AudioBuffers;
use audio_ports_config::AudioPortsConfig;
use clack_host::prelude::*;
use generic_daw_utils::unique_id;
use host::Host;
use main_thread::MainThread;
use shared::Shared;
use std::{collections::BTreeMap, ffi::CString, num::NonZero, path::PathBuf, result::Result};
use walkdir::WalkDir;

mod audio_buffers;
mod audio_ports_config;
mod audio_processor;
mod gui_ext;
mod host;
mod main_thread;
mod note_buffers;
mod plugin_descriptor;
mod plugin_type;
mod shared;
mod timer_ext;

pub use audio_processor::AudioProcessor;
pub use clack_host;
pub use gui_ext::GuiExt;
pub use main_thread::MainThreadMessage;
pub use note_buffers::NoteBuffers;
pub use plugin_descriptor::PluginDescriptor;
pub use plugin_id::Id as PluginId;
pub use plugin_type::PluginType;

unique_id!(plugin_id);

#[must_use]
pub fn get_installed_plugins() -> BTreeMap<PluginDescriptor, PluginBundle> {
    let mut r = BTreeMap::new();

    standard_clap_paths()
        .into_iter()
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

    #[cfg(target_os = "linux")]
    {
        if let Some(path) = std::env::var_os("HOME").map(PathBuf::from) {
            paths.push(path.join(".clap"));
        }

        paths.push("/usr/lib/clap".into());
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(path) = std::env::var_os("COMMONPROGRAMFILES").map(PathBuf::from) {
            paths.push(path.join("CLAP"));
        }

        if let Some(path) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            paths.push(path.join("Programs\\Common\\CLAP"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        paths.push("/Library/Audio/Plug-Ins/CLAP".into());

        if let Some(path) = std::env::var_os("HOME").map(PathBuf::from) {
            paths.push(path.join("Library/Audio/Plug-Ins/CLAP"));
        }
    }

    if let Some(clap_path) = std::env::var_os("CLAP_PATH") {
        paths.extend(std::env::split_paths(&clap_path));
    }

    paths
}

#[must_use]
pub fn init(
    bundle: &PluginBundle,
    descriptor: PluginDescriptor,
    sample_rate: f64,
    max_buffer_size: u32,
) -> (GuiExt, Receiver<MainThreadMessage>, AudioProcessor) {
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

    let latency = instance
        .access_handler(|mt: &MainThread<'_>| mt.latency)
        .map(|ext| ext.get(&mut instance.plugin_handle()))
        .and_then(NonZero::new);

    let audio_buffers = AudioBuffers::new(config, input_config, output_config, latency);
    let note_buffers = NoteBuffers::new(&mut instance.plugin_handle());
    let id = PluginId::unique();

    let plugin_audio_processor = AudioProcessor::new(
        instance
            .activate(|_, _| {}, config)
            .unwrap()
            .start_processing()
            .unwrap(),
        descriptor.ty,
        id,
        audio_buffers,
        note_buffers,
    );

    let gui = GuiExt::new(
        instance.access_handler(|mt| mt.gui).unwrap().0,
        instance,
        descriptor,
        id,
    );

    (gui, gui_receiver, plugin_audio_processor)
}
