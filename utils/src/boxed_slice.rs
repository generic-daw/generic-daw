#[macro_export]
macro_rules! boxed_slice {
	($elem:expr; $n:expr) => {
		vec![$elem; $n].into_boxed_slice()
	};
	($($x:expr),+ $(,)?) => {
		vec![$($x),+].into_boxed_slice()
	}
}
