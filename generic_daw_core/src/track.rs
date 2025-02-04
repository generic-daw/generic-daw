use crate::{Meter, Position, TrackClip};
use audio_graph::{AudioGraphNodeImpl, MixerNode};
use std::sync::{atomic::Ordering::Acquire, Arc};

pub mod dirty_event;

pub use dirty_event::DirtyEvent;

#[derive(Clone, Copy, Debug)]
pub enum TrackInner {
    Audio,
}

#[derive(Clone, Debug)]
pub struct Track {
    inner: TrackInner,
    /// contains audio clips for audio tracks, and midi patterns for midi tracks
    pub clips: Vec<Arc<TrackClip>>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// volume and pan
    pub node: Arc<MixerNode>,
}

impl AudioGraphNodeImpl for Track {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        if matches!(self.inner, TrackInner::Audio) && !self.meter.playing.load(Acquire) {
            return;
        }

        self.clips
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
    pub fn audio(meter: Arc<Meter>, node: Arc<MixerNode>) -> Self {
        Self::new(TrackInner::Audio, meter, node)
    }

    fn new(inner: TrackInner, meter: Arc<Meter>, node: Arc<MixerNode>) -> Self {
        Self {
            inner,
            clips: Vec::new(),
            meter,
            node,
        }
    }

    #[must_use]
    pub fn len(&self) -> Position {
        self.clips
            .iter()
            .map(|track| track.len())
            .max()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn try_push(&mut self, clip: &Arc<TrackClip>) -> bool {
        match self.inner {
            TrackInner::Audio => {
                if !matches!(**clip, TrackClip::Audio(..)) {
                    return false;
                };
            }
        }

        self.clips.push(clip.clone());

        true
    }
}
