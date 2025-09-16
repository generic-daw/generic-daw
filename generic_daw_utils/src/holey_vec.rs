use std::{fmt::Debug, ops::Index};

pub struct HoleyVec<T>(Vec<Option<T>>);

impl<T: Debug> Debug for HoleyVec<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_map().entries(self.iter()).finish()
	}
}

impl<T> Default for HoleyVec<T> {
	fn default() -> Self {
		Self(Vec::new())
	}
}

impl<T> Index<usize> for HoleyVec<T> {
	type Output = T;

	fn index(&self, index: usize) -> &Self::Output {
		self.get(index).unwrap()
	}
}

impl<T> HoleyVec<T> {
	#[must_use]
	pub fn get(&self, index: usize) -> Option<&T> {
		self.0.get(index).and_then(Option::as_ref)
	}

	#[must_use]
	pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
		self.0.get_mut(index).and_then(Option::as_mut)
	}

	pub fn insert(&mut self, index: usize, elem: T) -> Option<T> {
		self.entry(index).replace(elem)
	}

	pub fn remove(&mut self, index: usize) -> Option<T> {
		self.entry(index).take()
	}

	#[must_use]
	pub fn entry(&mut self, index: usize) -> &mut Option<T> {
		if index >= self.0.len() {
			self.0.resize_with(index + 1, || None);
		}

		&mut self.0[index]
	}

	#[must_use]
	pub fn contains_key(&self, key: usize) -> bool {
		self.get(key).is_some()
	}

	pub fn clear(&mut self) {
		self.0.clear();
	}

	pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> {
		self.0
			.iter()
			.enumerate()
			.filter_map(|(k, v)| Some((k, v.as_ref()?)))
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut T)> {
		self.0
			.iter_mut()
			.enumerate()
			.filter_map(|(k, v)| Some((k, v.as_mut()?)))
	}

	pub fn keys(&self) -> impl Iterator<Item = usize> {
		self.iter().map(|(k, _)| k)
	}

	pub fn values(&self) -> impl Iterator<Item = &T> {
		self.iter().map(|(_, v)| v)
	}

	pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
		self.iter_mut().map(|(_, v)| v)
	}
}

impl<T> HoleyVec<T>
where
	T: PartialEq,
{
	#[must_use]
	pub fn key_of(&self, value: &T) -> Option<usize> {
		self.iter().find_map(|(k, v)| (value == v).then_some(k))
	}

	#[must_use]
	pub fn contains_value(&self, value: &T) -> bool {
		self.key_of(value).is_some()
	}
}
