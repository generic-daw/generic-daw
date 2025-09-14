pub trait FunnelShiftExt<T> {
	fn funnel_shift_left(&mut self, other: &mut [T]);
}

impl<T> FunnelShiftExt<T> for [T] {
	fn funnel_shift_left(&mut self, other: &mut [T]) {
		if self.len() < other.len() {
			other.rotate_right(self.len());
			other[..self.len()].swap_with_slice(self);
		} else {
			self[..other.len()].swap_with_slice(other);
			self.rotate_left(other.len());
		}
	}
}
