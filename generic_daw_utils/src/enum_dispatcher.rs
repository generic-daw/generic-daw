use std::iter::FusedIterator;

#[derive(Clone, Copy, Debug)]
pub enum EnumDispatcher<A, B> {
	A(A),
	B(B),
}

impl<T, A, B> Iterator for EnumDispatcher<A, B>
where
	A: Iterator<Item = T>,
	B: Iterator<Item = T>,
{
	type Item = T;

	fn next(&mut self) -> Option<Self::Item> {
		match self {
			Self::A(a) => a.next(),
			Self::B(b) => b.next(),
		}
	}
}

impl<T, A, B> DoubleEndedIterator for EnumDispatcher<A, B>
where
	A: DoubleEndedIterator<Item = T>,
	B: DoubleEndedIterator<Item = T>,
{
	fn next_back(&mut self) -> Option<Self::Item> {
		match self {
			Self::A(a) => a.next_back(),
			Self::B(b) => b.next_back(),
		}
	}
}

impl<T, A, B> ExactSizeIterator for EnumDispatcher<A, B>
where
	A: ExactSizeIterator<Item = T>,
	B: ExactSizeIterator<Item = T>,
{
	fn len(&self) -> usize {
		match self {
			Self::A(a) => a.len(),
			Self::B(b) => b.len(),
		}
	}
}

impl<T, A, B> FusedIterator for EnumDispatcher<A, B>
where
	A: FusedIterator<Item = T>,
	B: FusedIterator<Item = T>,
{
}
