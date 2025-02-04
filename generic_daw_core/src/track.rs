use crate::{Meter, Position, TrackClip};
use atomig::Atomic;
use audio_graph::{AudioGraphNodeImpl, MixerNode};
use clap_host::PluginAudioProcessor;
use plugin_state::PluginState;
use std::sync::{atomic::Ordering::Acquire, Arc, Mutex, RwLock};

pub mod dirty_event;
pub mod plugin_state;

pub use dirty_event::DirtyEvent;

#[derive(Debug)]
pub enum TrackInner {
    Audio,
    Midi(Mutex<PluginState>),
}

#[derive(Debug)]
pub struct Track {
    inner: TrackInner,
    /// contains audio clips for audio tracks, and midi patterns for midi tracks
    pub clips: RwLock<Vec<Arc<TrackClip>>>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// volume and pan
    pub node: MixerNode,
}

impl AudioGraphNodeImpl for Track {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        if matches!(self.inner, TrackInner::Audio) && !self.meter.playing.load(Acquire) {
            return;
        }

        self.clips
            .read()
            .unwrap()
            .iter()
            .for_each(|clip| clip.fill_buf(buf_start_sample, buf));

        self.node.fill_buf(buf_start_sample, buf);
    }

    fn id(&self) -> audio_graph::NodeId {
        self.node.id()
    }
}

impl Track {
    #[must_use]
    pub fn audio(meter: Arc<Meter>) -> Arc<Self> {
        Arc::new(Self::new(TrackInner::Audio, meter))
    }

    #[must_use]
    pub fn midi(plugin: PluginAudioProcessor, meter: Arc<Meter>) -> Arc<Self> {
        Arc::new(Self::new(
            TrackInner::Midi(PluginState::create(plugin)),
            meter,
        ))
    }

    fn new(inner: TrackInner, meter: Arc<Meter>) -> Self {
        Self {
            inner,
            clips: RwLock::default(),
            meter,
            node: MixerNode::default(),
        }
    }

    #[must_use]
    pub fn len(&self) -> Position {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(|track| track.len())
            .max()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn try_push(&self, clip: &Arc<TrackClip>) -> bool {
        match self.inner {
            TrackInner::Audio => {
                if !matches!(**clip, TrackClip::Audio(..)) {
                    return false;
                };
            }
            TrackInner::Midi(..) => {
                if !matches!(**clip, TrackClip::Midi(..)) {
                    return false;
                };
            }
        }

        self.clips.write().unwrap().push(clip.clone());

        true
    }

    pub(crate) fn dirty(&self) -> Arc<Atomic<DirtyEvent>> {
        let TrackInner::Midi(plugin_state) = &self.inner else {
            unreachable!()
        };

        plugin_state.lock().unwrap().dirty.clone()
    }
}
