fn main() {
    tonic_build::compile_protos("proto/subblock.proto").unwrap();
}
