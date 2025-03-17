pub trait ShiftMoveExt {
    fn shift_move(&mut self, from: usize, to: usize);
}

impl<T> ShiftMoveExt for [T] {
    fn shift_move(&mut self, from: usize, to: usize) {
        if from > to {
            self[to..=from].rotate_right(1);
        } else {
            self[from..=to].rotate_left(1);
        }
    }
}
