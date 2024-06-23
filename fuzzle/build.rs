fn main() {
    // trigger recompilation when a new migration is added
    println!("cargo:rerun-if-changed=migrations");

    let proto_file = "../inference/inference.proto";
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .out_dir("./src/inference")
        .compile(&[proto_file], &[".."])
        .unwrap_or_else(|e| panic!("protobuf compile error: {}", e));
    println!("cargo:rerun-if-changed={}", proto_file);
}
