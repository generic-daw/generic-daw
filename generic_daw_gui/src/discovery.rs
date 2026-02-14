use crate::config::Config;
use generic_daw_core::clap_host::{DEFAULT_CLAP_PATHS, PluginDescriptor, get_installed_plugins};
use std::{
	env,
	ffi::CStr,
	io::{self, Read, Write, stdin, stdout},
	process::{Command, Stdio},
	sync::Arc,
};

pub fn discover_plugins_send() -> Result<(), Option<io::Error>> {
	let mut stdin = stdin();
	let mut stdout = stdout();

	let mut clap_paths = Vec::new();

	let mut buf1 = 0usize.to_ne_bytes();
	let mut buf2 = Vec::new();

	stdin.read_exact(&mut buf1)?;

	for _ in 0..usize::from_ne_bytes(buf1) {
		stdin.read_exact(&mut buf1)?;
		buf2.resize(usize::from_ne_bytes(buf1), 0);
		stdin.read_exact(&mut buf2)?;
		clap_paths.push(str::from_utf8(&buf2).map_err(|_| None)?.to_owned());
	}

	get_installed_plugins(&clap_paths, |descriptor| {
		_ = encode_descriptor(&descriptor, &mut stdout);
	});

	Ok(())
}

pub fn discover_plugins_recv(
	config: &Config,
	mut f: impl FnMut(PluginDescriptor),
) -> io::Result<()> {
	let mut child = Command::new(env::current_exe()?)
		.arg("--discover")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()?;

	let mut stdin = child.stdin.take().unwrap();
	let mut stdout = child.stdout.take().unwrap();

	stdin.write_all(&(DEFAULT_CLAP_PATHS.len() + config.clap_paths.len()).to_ne_bytes())?;

	for path in DEFAULT_CLAP_PATHS.iter().chain(&config.clap_paths) {
		if let Some(path) = path.to_str() {
			stdin.write_all(&path.len().to_ne_bytes())?;
			stdin.write_all(path.as_bytes())?;
		}
	}

	loop {
		match decode_descriptor(&mut stdout) {
			Ok(descriptor) => f(descriptor),
			Err(Some(err)) => {
				child.wait()?;
				return if err.kind() == io::ErrorKind::UnexpectedEof {
					Ok(())
				} else {
					Err(err)
				};
			}
			Err(None) => {}
		}
	}
}

fn encode_descriptor(this: &PluginDescriptor, mut w: impl Write) -> io::Result<()> {
	w.write_all(&this.name.len().to_ne_bytes())?;
	w.write_all(this.name.as_bytes())?;

	w.write_all(&this.id.to_bytes_with_nul().len().to_ne_bytes())?;
	w.write_all(this.id.to_bytes_with_nul())?;

	w.write_all(&this.path.len().to_ne_bytes())?;
	w.write_all(this.path.as_bytes())?;

	Ok(())
}

fn decode_descriptor(mut r: impl Read) -> Result<PluginDescriptor, Option<io::Error>> {
	let mut buf1 = 0usize.to_ne_bytes();
	let mut buf2 = Vec::new();

	r.read_exact(&mut buf1)?;
	buf2.resize(usize::from_ne_bytes(buf1), 0);
	r.read_exact(&mut buf2)?;
	let name = str::from_utf8(&buf2).map(Arc::from);

	r.read_exact(&mut buf1)?;
	buf2.resize(usize::from_ne_bytes(buf1), 0);
	r.read_exact(&mut buf2)?;
	let id = CStr::from_bytes_with_nul(&buf2).map(Arc::from);

	r.read_exact(&mut buf1)?;
	buf2.resize(usize::from_ne_bytes(buf1), 0);
	r.read_exact(&mut buf2)?;
	let path = str::from_utf8(&buf2).map(Arc::from);

	Ok(PluginDescriptor {
		name: name.map_err(|_| None)?,
		id: id.map_err(|_| None)?,
		path: path.map_err(|_| None)?,
	})
}
