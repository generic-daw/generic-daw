use std::{hash::Hasher, io::Read};

pub fn hash_reader<H: Hasher + Default>(mut read: impl Read) -> u64 {
    let mut hasher = H::default();
    let mut buf = [0; 4096];
    let mut len;

    while {
        len = read.read(&mut buf).unwrap();
        len != 0
    } {
        hasher.write(&buf[..len]);
    }

    hasher.finish()
}
