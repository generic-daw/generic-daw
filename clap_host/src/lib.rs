use async_channel::Receiver;
use audio_buffers::AudioBuffers;
use audio_ports_config::AudioPortsConfig;
use clack_host::prelude::*;
use event_buffers::EventBuffers;
use generic_daw_utils::unique_id;
use host::Host;
use main_thread::MainThread;
use shared::Shared;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};
use walkdir::WalkDir;

mod audio_buffers;
mod audio_ports_config;
mod audio_processor;
mod event_buffers;
mod event_impl;
pub mod events;
mod gui_ext;
mod host;
mod main_thread;
mod plugin_descriptor;
mod shared;

pub use audio_processor::AudioProcessor;
pub use clack_host::bundle::PluginBundle;
pub use event_impl::EventImpl;
pub use gui_ext::GuiExt;
pub use main_thread::MainThreadMessage;
pub use plugin_descriptor::PluginDescriptor;
pub use plugin_id::Id as PluginId;

unique_id!(plugin_id);

#[must_use]
pub fn get_installed_plugins(
    paths: impl IntoIterator<Item: AsRef<Path>>,
) -> BTreeMap<PluginDescriptor, PluginBundle> {
    let mut seen = HashSet::new();
    let mut bundles = BTreeMap::new();

    paths
        .into_iter()
        .flat_map(WalkDir::new)
        .flatten()
        .filter(|dir_entry| dir_entry.file_type().is_file())
        .filter(|dir_entry| {
            dir_entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "clap")
        })
        .filter_map(|path| {
            if seen.contains(path.path()) {
                None
            } else {
                // SAFETY:
                // loading an external library object file is inherently unsafe
                let bundle = unsafe { PluginBundle::load(path.path()).ok() };
                seen.insert(path.into_path());
                bundle
            }
        })
        .for_each(|bundle| {
            if let Some(factory) = bundle.get_plugin_factory() {
                factory
                    .plugin_descriptors()
                    .filter_map(|d| d.try_into().ok())
                    .for_each(|d| {
                        bundles.insert(d, bundle.clone());
                    });
            }
        });

    bundles
}

pub fn default_clap_paths() -> Vec<Arc<Path>> {
    let mut paths = Vec::new();

    #[cfg(target_os = "linux")]
    {
        if let Some(path) = std::env::var_os("HOME").map(PathBuf::from) {
            paths.push(path.join(".clap").into());
        }

        paths.push(Path::new("/usr/lib/clap").into());
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(path) = std::env::var_os("COMMONPROGRAMFILES").map(PathBuf::from) {
            paths.push(path.join("CLAP").into());
        }

        if let Some(path) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            paths.push(path.join("Programs\\Common\\CLAP").into());
        }
    }

    #[cfg(target_os = "macos")]
    {
        paths.push(Path::new("/Library/Audio/Plug-Ins/CLAP").into());

        if let Some(path) = std::env::var_os("HOME").map(PathBuf::from) {
            paths.push(path.join("Library/Audio/Plug-Ins/CLAP").into());
        }
    }

    if let Some(clap_path) = std::env::var_os("CLAP_PATH") {
        paths.extend(std::env::split_paths(&clap_path).map(Arc::from));
    }

    paths
}

#[must_use]
pub fn init(
    bundle: &PluginBundle,
    descriptor: PluginDescriptor,
    sample_rate: u32,
    max_buffer_size: u32,
) -> (GuiExt, Receiver<MainThreadMessage>, AudioProcessor) {
    let (main_sender, main_receiver) = async_channel::unbounded();
    let (audio_sender, audio_receiver) = async_channel::unbounded();

    let mut instance = PluginInstance::new(
        |()| Shared::new(descriptor.clone(), main_sender, audio_sender),
        |shared| MainThread::new(shared),
        bundle,
        &descriptor.id,
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
        sample_rate: f64::from(sample_rate),
        min_frames_count: 1,
        max_frames_count,
    };

    let latency = instance
        .access_handler(|mt: &MainThread<'_>| mt.latency)
        .map(|ext| ext.get(&mut instance.plugin_handle()))
        .unwrap_or_default();

    let audio_buffers = AudioBuffers::new(config, input_config, output_config, latency);
    let note_buffers = EventBuffers::new(&mut instance.plugin_handle());
    let id = PluginId::unique();

    let plugin_audio_processor = AudioProcessor::new(
        instance
            .activate(|_, _| {}, config)
            .unwrap()
            .start_processing()
            .unwrap(),
        descriptor.clone(),
        id,
        audio_buffers,
        note_buffers,
        audio_receiver,
    );

    let gui = GuiExt::new(
        instance.access_handler(|mt| mt.gui).unwrap().0,
        instance,
        descriptor,
        id,
    );

    (gui, main_receiver, plugin_audio_processor)
}
