fn main() {
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path().expect("vendored protoc"));
    config
        .compile_protos(&["../../proto/deskharbor.proto"], &["../../proto"])
        .expect("protocol schema");
    println!("cargo:rerun-if-changed=../../proto/deskharbor.proto");
}
