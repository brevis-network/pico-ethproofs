fn main() {
    tonic_build::compile_protos("proto/proof.proto").unwrap();
}
