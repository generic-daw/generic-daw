use crate::track::Track;
use cpal::StreamConfig;
use hound::WavWriter;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

// Structure to manage an arrangement of audio clips
pub struct Arrangement {
    tracks: Vec<Arc<Mutex<Track>>>,
}

impl Default for Arrangement {
    fn default() -> Self {
        Self::new()
    }
}

impl Arrangement {
    pub const fn new() -> Self {
        Self { tracks: Vec::new() }
    }

    // Method to get the combined audio sample at a specific global time
    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        self.tracks
            .iter()
            .map(|track| track.lock().unwrap().get_at_global_time(global_time))
            .sum::<f32>()
            .clamp(-1.0, 1.0) // Clip the output
    }

    // Method to get the length of the arrangement in tracks
    pub fn len_tracks(&self) -> u32 {
        self.tracks.len() as u32
    }

    // Method to get the length of the arrangement in samples
    pub fn len_samples(&self) -> u32 {
        self.tracks
            .iter()
            .map(|track| track.lock().unwrap().len())
            .max()
            .unwrap()
    }

    // Method to check if the arrangement is empty
    pub fn is_empty(&self) -> bool {
        self.len_samples() == 0
    }

    // Method to add a track to the arrangement
    pub fn push(&mut self, track: Track) {
        self.tracks.push(Arc::new(Mutex::new(track)));
    }

    // Method to remove a track from the arrangement
    pub fn remove(&mut self, index: usize) {
        self.tracks.remove(index);
    }

    // Method to get the track at an index
    pub fn get(&self, index: u32) -> &Arc<Mutex<Track>> {
        &self.tracks[index as usize]
    }

    // Method to export the arrangement to a WAV file
    pub fn export(&self, path: &Path, config: &StreamConfig) {
        let mut writer = WavWriter::create(
            path,
            hound::WavSpec {
                channels: config.channels,
                sample_rate: config.sample_rate.0,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();

        for i in 0..self.len_samples() {
            writer.write_sample(self.get_at_global_time(i)).unwrap();
        }
    }
}
