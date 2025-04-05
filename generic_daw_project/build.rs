fn main() {
    println!("cargo::rerun-if-changed=src/project.proto");
    prost_build::compile_protos(&["src/project.proto"], &["src/"]).unwrap();
}
