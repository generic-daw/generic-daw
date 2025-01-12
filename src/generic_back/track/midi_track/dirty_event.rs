use atomig::Atom;

#[repr(u8)]
#[derive(Atom, Clone, Copy, Debug, Default)]
pub enum DirtyEvent {
    #[default]
    None,
    NoteAdded,
    NoteRemoved,
    NoteReplaced,
}
