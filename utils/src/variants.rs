#[macro_export]
macro_rules! variants {
	(
		$(#[$meta:meta])*
		$vis:vis enum $ident:ident {
			$(
				$(#[$variant_meta:meta])*
				$variant:ident,
			)+
		}
	) => {
		$(#[$meta])*
		$vis enum $ident {
			$(
				$(#[$variant_meta])*
				$variant,
			)+
		}

		impl $ident {
			pub const VARIANTS: &[Self] = &[$(Self::$variant,)+];
		}
	};
}
