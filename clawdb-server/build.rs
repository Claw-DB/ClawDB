fn main() -> Result<(), Box<dyn std::error::Error>> {
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
