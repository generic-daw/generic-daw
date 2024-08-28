use anyhow::Result;
use cpal::{traits::StreamTrait, Device, Stream, StreamConfig};
use generic_back::{
    arrangement::Arrangement,
    cpal_get_default_device, get_output_stream,
    track::Track,
    track_clip::audio_clip::{read_audio_file, AudioClip, InterleavedAudio},
};
use std::{path::PathBuf, sync::Arc, sync::Mutex};

pub struct AudioEngine {
    device: Device,
    config: StreamConfig,
    arrangement: Arc<Mutex<Arrangement>>, // Use Mutex here
    stream: Option<Stream>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (device, config) = cpal_get_default_device();
        let arrangement = Arc::new(Mutex::new(Arrangement::new()));

        Self {
            device,
            config,
            arrangement,
            stream: None,
        }
    }

    pub fn load_sample(&self, path: &str) -> Result<Arc<InterleavedAudio>> {
        read_audio_file(&PathBuf::from(path), &self.config)
    }

    pub fn add_track(&self) -> u32 {
        self.arrangement.lock().unwrap().push(Track::new());
        self.arrangement.lock().unwrap().len_tracks() - 1
    }

    pub fn add_audio_clip(&self, index: u32, clip: Arc<AudioClip>) {
        self.arrangement
            .lock()
            .unwrap()
            .get(index)
            .lock()
            .unwrap()
            .push(clip);
    }

    pub fn play(&mut self) {
        if self.stream.is_none() {
            let stream = get_output_stream(&self.device, &self.config, self.arrangement.clone(), 0);
            stream.play().unwrap();
            self.stream = Some(stream);
        }
    }

    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
    }
}
