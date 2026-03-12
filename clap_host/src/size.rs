use crate::API_TYPE;

#[derive(Clone, Copy, Debug)]
pub enum Size {
	Logical { width: f32, height: f32 },
	Physical { width: f32, height: f32 },
}

impl Size {
	#[must_use]
	pub fn from_logical((width, height): (f32, f32)) -> Self {
		Self::Logical { width, height }
	}

	#[must_use]
	pub fn to_logical(self, scale_factor: f32) -> (f32, f32) {
		let (width, height) = match self {
			Self::Logical { width, height } => (width, height),
			Self::Physical { width, height } => (width / scale_factor, height / scale_factor),
		};
		(width, height)
	}

	#[must_use]
	pub fn from_physical((width, height): (f32, f32)) -> Self {
		Self::Physical { width, height }
	}

	#[must_use]
	pub fn to_physical(self, scale_factor: f32) -> (f32, f32) {
		let (width, height) = match self {
			Self::Logical { width, height } => (width * scale_factor, height * scale_factor),
			Self::Physical { width, height } => (width, height),
		};
		(width, height)
	}

	#[must_use]
	pub fn from_native((width, height): (f32, f32)) -> Self {
		if API_TYPE.uses_logical_size() {
			Self::Logical { width, height }
		} else {
			Self::Physical { width, height }
		}
	}

	#[must_use]
	pub fn to_native(self, scale_factor: f32) -> (f32, f32) {
		if API_TYPE.uses_logical_size() {
			self.to_logical(scale_factor)
		} else {
			self.to_physical(scale_factor)
		}
	}

	#[must_use]
	pub fn approx_eq(self, other: Self, scale_factor: f32) -> bool {
		let (lw, lh) = self.to_logical(scale_factor);
		let (rw, rh) = other.to_logical(scale_factor);
		(lw - rw).abs() <= 1.0 && (lh - rh).abs() <= 1.0
	}
}
