use audio_processor::AudioProcessor;
use clack_host::prelude::*;
use gui::GuiExt;
use home::home_dir;
use host::{Host, HostThreadMessage};
use main_thread::{MainThread, MainThreadMessage};
use shared::Shared;
use std::{
    cell::UnsafeCell,
    future::Future,
    marker::PhantomData,
    path::PathBuf,
    result::Result,
    sync::mpsc::{Receiver, Sender},
};
use walkdir::WalkDir;
use winit::raw_window_handle::RawWindowHandle;

mod audio_processor;
mod gui;
mod host;
mod main_thread;
mod shared;
mod timer;

#[derive(Debug)]
pub struct ClapPlugin {
    sender: Sender<MainThreadMessage>,
    receiver: Receiver<HostThreadMessage>,
    _no_sync: PhantomData<UnsafeCell<()>>,
}

impl ClapPlugin {
    fn new(sender: Sender<MainThreadMessage>, receiver: Receiver<HostThreadMessage>) -> Self {
        Self {
            sender,
            receiver,
            _no_sync: PhantomData,
        }
    }

    pub fn process_audio(
        &self,
        input_audio: Vec<Vec<f32>>,
        input_audio_ports: AudioPorts,
        output_audio_ports: AudioPorts,
        input_events: EventBuffer,
    ) -> (Vec<Vec<f32>>, EventBuffer) {
        self.sender
            .send(MainThreadMessage::ProcessAudio(
                input_audio,
                input_audio_ports,
                output_audio_ports,
                input_events,
            ))
            .unwrap();

        match self.receiver.recv() {
            Ok(HostThreadMessage::AudioProcessed(output_audio, output_events)) => {
                (output_audio, output_events)
            }
            _ => unreachable!(),
        }
    }

    pub fn get_counter(&self) -> u64 {
        self.sender.send(MainThreadMessage::GetCounter).unwrap();

        match self.receiver.recv() {
            Ok(HostThreadMessage::Counter(counter)) => counter,
            _ => unreachable!(),
        }
    }

    pub fn get_state(&self) -> Vec<u8> {
        self.sender.send(MainThreadMessage::GetState).unwrap();

        match self.receiver.recv() {
            Ok(HostThreadMessage::State(state)) => state,
            _ => unreachable!(),
        }
    }

    pub fn set_state(&self, state: Vec<u8>) {
        self.sender
            .send(MainThreadMessage::SetState(state))
            .unwrap();
    }
}

#[expect(dead_code)]
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

#[expect(dead_code)]
pub fn run(
    bundle: &PluginBundle,
    config: PluginAudioConfiguration,
    window_handle: RawWindowHandle,
) -> (ClapPlugin, impl Future + use<>) {
    let (sender_plugin, receiver_plugin) = std::sync::mpsc::channel();
    let (sender_host, receiver_host) = std::sync::mpsc::channel();

    let sender_plugin_clone = sender_plugin.clone();

    let factory = bundle.get_plugin_factory().unwrap();
    let plugin_descriptor = factory.plugin_descriptors().next().unwrap();
    let mut instance = PluginInstance::<Host>::new(
        |()| Shared::new(sender_plugin_clone),
        |shared| MainThread::new(shared),
        bundle,
        plugin_descriptor.id().unwrap(),
        &HostInfo::new("", "", "", "").unwrap(),
    )
    .unwrap();

    let audio_processor = instance
        .activate(|_, _| {}, config)
        .unwrap()
        .start_processing()
        .unwrap();

    let mut gui = instance
        .access_handler(|h| h.gui)
        .map(|gui| GuiExt::new(gui, &mut instance.plugin_handle()))
        .unwrap();

    if gui.needs_floating().unwrap() {
        gui.open_floating(&mut instance.plugin_handle());
    } else {
        gui.open_embedded(&mut instance.plugin_handle(), window_handle);
    };

    (
        ClapPlugin::new(sender_plugin, receiver_host),
        gui.run(
            instance,
            sender_host,
            receiver_plugin,
            AudioProcessor::new(audio_processor),
        ),
    )
}
