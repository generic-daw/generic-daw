#[derive(Debug)]
pub struct PushSlot<T> {
	t: Option<T>,
	r: oneshot::Receiver<Self>,
}

impl<T> PushSlot<T> {
	pub fn new(t: Option<T>, r: oneshot::Receiver<Self>) -> Self {
		Self { t, r }
	}

	pub fn try_recv(&mut self) -> Option<&mut T> {
		if self.t.is_none()
			&& let Ok(t) = self.r.try_recv()
		{
			*self = t;
		}

		self.as_mut()
	}

	pub fn recv(&mut self) -> Option<&mut T> {
		if self.t.is_none()
			&& let Ok(t) = self.r.recv_ref()
		{
			*self = t;
		}

		self.as_mut()
	}

	pub fn as_mut(&mut self) -> Option<&mut T> {
		self.t.as_mut()
	}

	pub fn take(&mut self) -> Option<T> {
		self.t.take()
	}
}

#[derive(Debug)]
pub enum PullSlot<T> {
	Full(T),
	Empty(oneshot::Receiver<T>),
}

impl<T> PullSlot<T> {
	pub fn try_recv(&mut self) -> Option<&mut T> {
		match self {
			Self::Full(t) => Some(t),
			Self::Empty(t) => t.try_recv().map_or_default(|t| {
				*self = Self::Full(t);
				self.try_recv()
			}),
		}
	}

	pub fn recv(&mut self) -> Option<&mut T> {
		match self {
			Self::Full(t) => Some(t),
			Self::Empty(t) => t.recv_ref().map_or_default(|t| {
				*self = Self::Full(t);
				self.try_recv()
			}),
		}
	}
}
