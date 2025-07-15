#[macro_export]
macro_rules! include_f32s {
	($file:expr) => {{
		#[repr(align(4))]
		struct Align<T: ?Sized>(T);
		static ALIGNED: &Align<[u8]> = &Align(*::core::include_bytes!($file));
		assert!(ALIGNED.0.len().is_multiple_of(4));
		// SAFETY:
		// Every valid [u8; 4] bitpattern is a valid f32 bitpattern. The resulting
		// slice is correctly aligned to 4 bytes and inhabits exactly the same memory
		// as the original slice.
		unsafe { ::std::slice::from_raw_parts(ALIGNED.0.as_ptr().cast(), ALIGNED.0.len() / 4) }
	}};
}
