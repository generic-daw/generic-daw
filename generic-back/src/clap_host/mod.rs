mod gui;
mod host_shared;

use clack_extensions::gui::{GuiSize, HostGui};
use clack_host::{prelude::*, process::StartedPluginAudioProcessor};
use cpal::StreamConfig;
use gui::Gui;
use host_shared::{HostPluginThread, HostShared};
use std::{
    path::PathBuf,
    result::Result,
    sync::{
        atomic::{AtomicU32, Ordering::SeqCst},
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
};
use walkdir::WalkDir;

pub enum PluginThreadMessage {
    RunOnMainThread,
    GuiClosed,
    GuiRequestResized(GuiSize),
    ProcessAudio(
        [[f32; 8]; 2],
        Arc<Mutex<AudioPorts>>,
        Arc<Mutex<AudioPorts>>,
        EventBuffer,
        EventBuffer,
    ),
    GetCounter,
}

pub enum HostThreadMessage {
    AudioProcessed([[f32; 8]; 2], EventBuffer),
    Counter(u32),
}

pub struct Host;

impl HostHandlers for Host {
    type Shared<'a> = HostShared;
    type MainThread<'a> = HostPluginThread<'a>;
    type AudioProcessor<'a> = ();

    fn declare_extensions(builder: &mut HostExtensions<Self>, _shared: &Self::Shared<'_>) {
        builder.register::<HostGui>();
    }
}

struct StreamPluginAudioProcessor {
    audio_processor: Mutex<StartedPluginAudioProcessor<Host>>,
    plugin_sample_counter: AtomicU32,
}

impl StreamPluginAudioProcessor {
    fn process(
        &self,
        input_audio: &InputAudioBuffers,
        output_audio: &mut OutputAudioBuffers,
        input_events: &InputEvents,
        output_events: &mut OutputEvents,
    ) {
        self.audio_processor
            .lock()
            .unwrap()
            .process(
                input_audio,
                output_audio,
                input_events,
                output_events,
                Some(self.get_counter() as u64),
                None,
            )
            .unwrap();

        let current_counter = self.plugin_sample_counter.load(SeqCst);
        self.plugin_sample_counter
            .store(current_counter + 1, SeqCst);
    }

    fn get_counter(&self) -> u32 {
        self.plugin_sample_counter.load(SeqCst)
    }
}

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

    if let Some(home_dir) = dirs::home_dir() {
        paths.push(home_dir.join(".clap"));

        #[cfg(target_os = "macos")]
        {
            paths.push(home_dir.join("Library/Audio/Plug-Ins/CLAP"));
        }
    }

    #[cfg(windows)]
    {
        if let Some(val) = std::env::var_os("CommonProgramFiles") {
            paths.push(PathBuf::from(val).join("CLAP"));
        }

        if let Some(dir) = dirs::config_local_dir() {
            paths.push(dir.join("Programs\\Common\\CLAP"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
    }

    #[cfg(target_family = "unix")]
    {
        paths.push("/usr/lib/clap".into());
    }

    if let Some(env_var) = std::env::var_os("CLAP_PATH") {
        paths.extend(std::env::split_paths(&env_var));
    }

    paths
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(
    bundle: PluginBundle,
    config: StreamConfig,
) -> (Sender<PluginThreadMessage>, Receiver<HostThreadMessage>) {
    let (sender_plugin, receiver_plugin) = std::sync::mpsc::channel();
    let (sender_host, receiver_host) = std::sync::mpsc::channel();

    let sender_plugin_clone = sender_plugin.clone();
    std::thread::spawn(move || {
        let factory = bundle.get_plugin_factory().unwrap();
        let plugin_descriptor = factory.plugin_descriptors().next().unwrap();
        let mut instance = PluginInstance::<Host>::new(
            |()| HostShared::new(sender_plugin_clone),
            |_| HostPluginThread::new(),
            &bundle,
            plugin_descriptor.id().unwrap(),
            &HostInfo::new("", "", "", "").unwrap(),
        )
        .unwrap();

        let audio_config = PluginAudioConfiguration {
            sample_rate: config.sample_rate.0 as f64,
            min_frames_count: 16,
            max_frames_count: 16,
        };

        let audio_processor = instance
            .activate(|_, _| {}, audio_config)
            .unwrap()
            .start_processing()
            .unwrap();

        let gui = instance
            .access_handler(|h| h.gui)
            .map(|gui| Gui::new(gui, &mut instance.plugin_handle()))
            .unwrap();

        if gui.needs_floating().unwrap() {
            run_gui_floating(
                instance,
                &sender_host,
                receiver_plugin,
                gui,
                &StreamPluginAudioProcessor {
                    audio_processor: Mutex::new(audio_processor),
                    plugin_sample_counter: AtomicU32::new(0),
                },
            );
        } else {
            run_gui_embedded(
                instance,
                &sender_host,
                receiver_plugin,
                gui,
                &StreamPluginAudioProcessor {
                    audio_processor: Mutex::new(audio_processor),
                    plugin_sample_counter: AtomicU32::new(0),
                },
            );
        }
    });

    (sender_plugin, receiver_host)
}

#[allow(clippy::needless_pass_by_value)]
fn run_gui_embedded(
    mut _instance: PluginInstance<Host>,
    _sender: &Sender<HostThreadMessage>,
    _receiver: Receiver<PluginThreadMessage>,
    mut _gui: Gui,
    _audio_processor: &StreamPluginAudioProcessor,
) {
    todo!()
}

#[allow(clippy::significant_drop_tightening)]
fn run_gui_floating(
    mut instance: PluginInstance<Host>,
    sender: &Sender<HostThreadMessage>,
    receiver: Receiver<PluginThreadMessage>,
    mut gui: Gui,
    audio_processor: &StreamPluginAudioProcessor,
) {
    gui.open_floating(&mut instance.plugin_handle()).unwrap();

    for message in receiver {
        match message {
            PluginThreadMessage::RunOnMainThread => instance.call_on_main_thread_callback(),
            PluginThreadMessage::GuiClosed { .. } => {
                println!("Window closed!");
                break;
            }
            PluginThreadMessage::GuiRequestResized(gui_size) => {
                gui.resize(
                    &mut instance.plugin_handle(),
                    gui.gui_size_to_winit_size(gui_size),
                    1.0f64,
                );
            }
            PluginThreadMessage::ProcessAudio(
                mut input_buffers,
                input_audio_ports,
                output_audio_ports,
                input_events,
                mut output_events,
            ) => {
                let mut input_audio_ports = input_audio_ports.lock().unwrap();
                let input_audio = input_audio_ports.with_input_buffers([AudioPortBuffer {
                    latency: 0,
                    channels: AudioPortBufferType::f32_input_only(
                        input_buffers.iter_mut().map(InputChannel::constant),
                    ),
                }]);

                let mut output_buffers = [[0.0; 8]; 2];
                let mut output_audio_ports = output_audio_ports.lock().unwrap();
                let mut output_audio = output_audio_ports.with_output_buffers([AudioPortBuffer {
                    latency: 0,
                    channels: AudioPortBufferType::f32_output_only(
                        output_buffers.iter_mut().map(<[f32; 8]>::as_mut_slice),
                    ),
                }]);

                audio_processor.process(
                    &input_audio,
                    &mut output_audio,
                    &InputEvents::from_buffer(&input_events),
                    &mut OutputEvents::from_buffer(&mut output_events),
                );

                sender
                    .send(HostThreadMessage::AudioProcessed(
                        output_buffers,
                        output_events,
                    ))
                    .unwrap();
            }
            PluginThreadMessage::GetCounter => {
                sender
                    .send(HostThreadMessage::Counter(audio_processor.get_counter()))
                    .unwrap();
            }
        }
    }

    gui.destroy(&mut instance.plugin_handle());
}
