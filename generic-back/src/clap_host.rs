use clack_host::{prelude::*, process::StartedPluginAudioProcessor};
use cpal::StreamConfig;
use std::{
    path::PathBuf,
    result::Result,
    sync::{
        atomic::{AtomicU32, Ordering::SeqCst},
        Mutex,
    },
};
use walkdir::WalkDir;

pub struct HostShared;

impl<'a> SharedHandler<'a> for HostShared {
    fn request_process(&self) {}
    fn request_callback(&self) {}
    fn request_restart(&self) {}
}

pub struct Host;

impl HostHandlers for Host {
    type Shared<'a> = HostShared;

    type MainThread<'a> = ();
    type AudioProcessor<'a> = ();
}

pub struct StreamPluginAudioProcessor {
    audio_processor: Mutex<StartedPluginAudioProcessor<Host>>,
    plugin_sample_counter: AtomicU32,
}

impl StreamPluginAudioProcessor {
    pub fn new(bundle: &PluginBundle, config: &StreamConfig) -> Self {
        let factory = bundle.get_plugin_factory().unwrap();
        let plugin_descriptor = factory.plugin_descriptors().next().unwrap();

        let mut instance = PluginInstance::<Host>::new(
            |()| HostShared,
            |_| (),
            bundle,
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
            .activate(|_, ()| {}, audio_config)
            .unwrap()
            .start_processing()
            .unwrap();

        Self {
            audio_processor: Mutex::new(audio_processor),
            plugin_sample_counter: AtomicU32::new(0),
        }
    }

    // process input audio and midi events, and write output audio
    // saved in chunks of 16 samples because that's the size of cpal's output buffer
    pub fn process(
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

    pub fn get_counter(&self) -> u32 {
        self.plugin_sample_counter.load(SeqCst)
    }

    pub fn reset_counter(&self) {
        self.plugin_sample_counter.store(0, SeqCst);
    }
}

// Returns a list of all the plugins installed in standard CLAP search paths
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

// Returns a list of all the standard CLAP search paths, per the CLAP specification.
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
