#[macro_export]
macro_rules! variants {
	(
		$(#[$meta:meta])*
		$vis:vis enum $name:ident {
			$(
				$(#[$field_meta:meta])*
				$variant:ident,
			)+
		}
	) => {
		$(#[$meta])*
		$vis enum $name {
			$(
				$(#[$field_meta])*
				$variant,
			)+
		}

		impl $name {
			pub const VARIANTS: &[Self] = &[$(Self::$variant,)+];
		}
	};
}
