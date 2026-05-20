#![expect(clippy::iter_on_single_items, clippy::iter_on_empty_collections)]

use std::{
	iter::{Chain, Flatten},
	option::IntoIter,
};

pub fn left<I, L: IntoIterator<Item = I>, R: IntoIterator<Item = I>>(
	l: L,
) -> Chain<Flatten<IntoIter<L>>, Flatten<IntoIter<R>>> {
	Some(l)
		.into_iter()
		.flatten()
		.chain(None::<R>.into_iter().flatten())
}

pub fn right<I, L: IntoIterator<Item = I>, R: IntoIterator<Item = I>>(
	r: R,
) -> Chain<Flatten<IntoIter<L>>, Flatten<IntoIter<R>>> {
	None::<L>
		.into_iter()
		.flatten()
		.chain(Some(r).into_iter().flatten())
}
