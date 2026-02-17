#[macro_export]
macro_rules! unique_id {
	($mod_name:ident) => {
		mod $mod_name {
			use ::core::{
				num::NonZero,
				sync::atomic::{AtomicUsize, Ordering::Relaxed},
			};

			static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

			#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
			pub struct Id(NonZero<usize>);

			impl Id {
				pub fn unique() -> Self {
					Self(NEXT_ID.fetch_add(1, Relaxed).try_into().unwrap())
				}
			}
		}
	};
}
