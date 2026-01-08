use std::{
	borrow::{Borrow, BorrowMut},
	fmt::{Debug, Formatter},
	ops::{Deref, DerefMut},
};

#[derive(Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NoClone<T>(pub T);

impl<T: Debug> Debug for NoClone<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(f)
	}
}

impl<T> Clone for NoClone<T> {
	fn clone(&self) -> Self {
		panic!();
	}
}

impl<T> AsRef<T> for NoClone<T> {
	fn as_ref(&self) -> &T {
		&self.0
	}
}

impl<T> AsMut<T> for NoClone<T> {
	fn as_mut(&mut self) -> &mut T {
		&mut self.0
	}
}

impl<T> Borrow<T> for NoClone<T> {
	fn borrow(&self) -> &T {
		&self.0
	}
}

impl<T> BorrowMut<T> for NoClone<T> {
	fn borrow_mut(&mut self) -> &mut T {
		&mut self.0
	}
}

impl<T> Deref for NoClone<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> DerefMut for NoClone<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<T> From<T> for NoClone<T> {
	fn from(value: T) -> Self {
		Self(value)
	}
}
