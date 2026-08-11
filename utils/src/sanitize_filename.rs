#[must_use]
pub fn sanitize_filename_chars(input: &str) -> String {
	input
		.chars()
		.map(|c| {
			if if cfg!(windows) {
				matches!(
					c,
					'<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'..='\x1f'
				)
			} else if cfg!(unix) {
				matches!(c, '\0' | '/')
			} else {
				false
			} {
				'_'
			} else {
				c
			}
		})
		.collect()
}

#[must_use]
pub fn sanitize_filename(input: &str) -> String {
	let mut output = sanitize_filename_chars(input);

	if cfg!(windows) {
		let base = output.split_once('.').map_or(&*output, |(base, _)| base);
		if [
			"CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
			"COM8", "COM9", "COM¹", "COM²", "COM³", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
			"LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³",
		]
		.into_iter()
		.any(|reserved| base.eq_ignore_ascii_case(reserved))
		{
			output.insert(0, '_');
		}

		if output.ends_with([' ', '.']) {
			output.push('_');
		}
	} else if cfg!(unix) && [".", ".."].into_iter().any(|reserved| input == reserved) {
		output.insert(0, '_');
	}

	if output.is_empty() {
		output.push('_');
	}

	output
}
