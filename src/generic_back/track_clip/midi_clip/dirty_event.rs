use atomic_enum::atomic_enum;

#[atomic_enum]
#[derive(PartialEq, Eq)]
pub(in crate::generic_back) enum DirtyEvent {
    // can we reasonably assume that only one of these will happen per sample?
    None,
    NoteAdded,
    NoteRemoved,
    NoteReplaced,
}
