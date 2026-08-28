#[macro_export]
macro_rules! variants {
	(
		$(#[$meta:meta])*
		$vis:vis enum $ident:ident {
			$(
				$(#[$variant_meta:meta])*
				$variant:ident $( = $expr:expr)?
			),+ $(,)?
		}
	) => {
		$(#[$meta])*
		$vis enum $ident {
			$(
				$(#[$variant_meta])*
				$variant $( = $expr)?,
			)+
		}

		impl $ident {
			pub const VARIANTS: &[Self] = &[$(Self::$variant,)+];
		}
	};

	(
		$(#[$meta:meta])*
		$vis:vis enum $ident:ident {}
	) => {
		$(#[$meta])*
		$vis enum $ident {}

		impl $ident {
			pub const VARIANTS: &[Self] = &[];
		}
	};
}
