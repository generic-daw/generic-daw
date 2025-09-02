use crate::API_TYPE;

#[derive(Clone, Copy, Debug)]
pub enum Size {
	Logical { width: f32, height: f32 },
	Physical { width: f32, height: f32 },
	Native { width: f32, height: f32 },
}

impl Size {
	#[must_use]
	pub fn from_logical((width, height): (f32, f32)) -> Self {
		Self::Logical { width, height }
	}

	#[must_use]
	pub fn to_logical(self, scale_factor: f32) -> (f32, f32) {
		match self {
			Self::Logical { width, height } => (width, height),
			Self::Physical { width, height } => (width * scale_factor, height * scale_factor),
			Self::Native { .. } => self.normalize().to_logical(scale_factor),
		}
	}

	#[must_use]
	pub fn from_physical((width, height): (f32, f32)) -> Self {
		Self::Physical { width, height }
	}

	#[must_use]
	pub fn to_physical(self, scale_factor: f32) -> (f32, f32) {
		match self {
			Self::Logical { width, height } => (width / scale_factor, height / scale_factor),
			Self::Physical { width, height } => (width, height),
			Self::Native { .. } => self.normalize().to_physical(scale_factor),
		}
	}

	#[must_use]
	pub fn from_native((width, height): (f32, f32)) -> Self {
		Self::Native { width, height }
	}

	#[must_use]
	pub fn to_native(self, scale_factor: f32) -> (f32, f32) {
		if API_TYPE.uses_logical_size() {
			self.to_logical(scale_factor)
		} else {
			self.to_physical(scale_factor)
		}
	}

	fn normalize(self) -> Self {
		if let Self::Native { width, height } = self {
			if API_TYPE.uses_logical_size() {
				Self::Logical { width, height }
			} else {
				Self::Physical { width, height }
			}
		} else {
			self
		}
	}
}
