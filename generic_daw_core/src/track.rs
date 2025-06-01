use crate::{Action, Clip, MixerNode, Position, daw_ctx::State, event::Event};
use audio_graph::{NodeId, NodeImpl};

#[derive(Debug, Default)]
pub struct Track {
    pub clips: Vec<Clip>,
    /// volume, pan and plugins
    pub node: MixerNode,
}

impl NodeImpl for Track {
    type Action = Action;
    type Event = Event;
    type State = State;

    fn apply(&mut self, action: Self::Action) {
        match action {
            Self::Action::AddClip(clip) => self.clips.push(clip),
            Self::Action::RemoveClip(index) => _ = self.clips.remove(index),
            action => self.node.apply(action),
        }
    }

    fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
        for clip in &self.clips {
            clip.process(&state.meter, audio, events);
        }

        self.node.process(state, audio, events);
    }

    fn id(&self) -> NodeId {
        self.node.id()
    }

    fn reset(&mut self) {
        self.node.reset();
    }

    fn delay(&self) -> usize {
        self.node.delay()
    }
}

impl Track {
    #[must_use]
    pub fn len(&self) -> Position {
        self.clips
            .iter()
            .map(|clip| clip.position().get_global_end())
            .max()
            .unwrap_or_default()
    }
}
