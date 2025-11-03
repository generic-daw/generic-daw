#[macro_export]
macro_rules! include_f32s {
	($file:expr $(,)?) => {
		const {
			const BYTES: &[u8] = ::core::include_bytes!($file);
			::core::assert!(BYTES.len().is_multiple_of(4));

			let mut f32s = [0.0; BYTES.len() / 4];
			let mut i = 0;

			while i < f32s.len() {
				f32s[i] = f32::from_le_bytes([
					BYTES[4 * i],
					BYTES[4 * i + 1],
					BYTES[4 * i + 2],
					BYTES[4 * i + 3],
				]);

				i += 1;
			}

			f32s
		}
	};
}
