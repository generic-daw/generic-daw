use std::{
    fs::File,
    hash::{DefaultHasher, Hash as _, Hasher as _},
    io::Read as _,
    path::Path,
};

pub fn hash_file(path: impl AsRef<Path>) -> u64 {
    let mut hasher = DefaultHasher::new();
    let mut buf = [0; 4096];
    let mut file = File::open(path).unwrap();
    let mut read;

    while {
        read = file.read(&mut buf).unwrap();
        read != 0
    } {
        buf[..read].hash(&mut hasher);
    }

    hasher.finish()
}
