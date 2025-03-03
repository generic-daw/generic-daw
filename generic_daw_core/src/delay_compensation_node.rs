use audio_graph::{AudioGraphNodeImpl, NodeId};
use std::cell::RefCell;

#[derive(Debug)]
pub struct DelayCompensationNode {
    id: NodeId,
    buf: RefCell<Box<[f32]>>,
}

impl AudioGraphNodeImpl for DelayCompensationNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn fill_buf(&self, buf: &mut [f32]) {
        let mut sus = self.buf.borrow_mut();

        if sus.len() < buf.len() {
            buf.rotate_right(sus.len());

            for (i, s) in buf.iter_mut().zip(&mut *sus) {
                (*i, *s) = (*s, *i);
            }
        } else {
            for (i, s) in buf.iter_mut().zip(&mut *sus) {
                (*i, *s) = (*s, *i);
            }

            sus.rotate_right(buf.len());
        }
    }
}

impl DelayCompensationNode {
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            id: NodeId::unique(),
            buf: RefCell::new(vec![0.0; size].into_boxed_slice()),
        }
    }
}
