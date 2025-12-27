#[macro_export]
macro_rules! boxed_slice {
	($elem:expr; $n:expr) => {
		vec![$elem; $n].into_boxed_slice()
	};
}
