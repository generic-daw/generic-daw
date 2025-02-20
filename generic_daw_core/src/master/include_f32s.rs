#[macro_export]
macro_rules! include_f32s {
    ($file:expr) => {{
        #[repr(align(4))]
        struct Align<T: ?Sized>(T);
        static ALIGNED: &Align<[u8]> = &Align(*::core::include_bytes!($file));
        // SAFETY:
        // the resulting slice is correctly aligned and strictly within the original slice
        unsafe { ::std::slice::from_raw_parts(ALIGNED.0.as_ptr().cast(), ALIGNED.0.len() / 4) }
    }};
}
