pub trait RotateConcatExt<T> {
	fn rotate_right_concat(&mut self, other: &mut [T]);
}

impl<T> RotateConcatExt<T> for [T] {
	fn rotate_right_concat(&mut self, other: &mut [T]) {
		if self.is_empty() || other.is_empty() {
		} else if self.len() < other.len() {
			other.rotate_right(self.len());

			for (i, s) in other.iter_mut().zip(&mut *self) {
				std::mem::swap(i, s);
			}
		} else {
			for (i, s) in other.iter_mut().zip(&mut *self) {
				std::mem::swap(i, s);
			}

			self.rotate_right(other.len());
		}
	}
}
