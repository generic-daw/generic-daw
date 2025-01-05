use atomic_enum::atomic_enum;

#[atomic_enum]
pub enum DirtyEvent {
    None,
    NoteAdded,
    NoteRemoved,
    NoteReplaced,
}
