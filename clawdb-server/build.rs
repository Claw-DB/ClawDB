fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc_path = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc_path);

    let out_dir = std::env::var("OUT_DIR")?;
    let descriptor_path = std::path::Path::new(&out_dir).join("clawdb_descriptor.bin");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(&["proto/clawdb.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto/clawdb.proto");
    Ok(())
}
