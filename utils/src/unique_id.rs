use std::sync::atomic::AtomicUsize;

#[doc(hidden)]
pub static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[macro_export]
macro_rules! unique_id {
	($ident:ident) => {
		#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
		pub struct $ident(::std::num::NonZero<usize>);

		impl $ident {
			pub fn unique() -> Self {
				Self(
					::std::num::NonZero::new(
						$crate::NEXT_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
					)
					.unwrap(),
				)
			}
		}
	};
}
