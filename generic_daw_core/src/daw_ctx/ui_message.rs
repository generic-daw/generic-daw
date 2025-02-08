use audio_graph::AudioGraph;

#[derive(Debug)]
pub enum UiMessage<T> {
    AudioGraph(T, AudioGraph),
}
