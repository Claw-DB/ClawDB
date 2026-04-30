fn main() {
    // Declare the custom cfg to suppress unexpected_cfg warnings.
    println!("cargo:rustc-check-cfg=cfg(proto_compiled)");
    // Attempt proto compilation; print a warning but do not fail if protoc is unavailable.
    let result = tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/clawdb.proto"], &["proto"]);
    match result {
        Ok(_) => println!("cargo:rustc-cfg=proto_compiled"),
        Err(e) => println!("cargo:warning=Proto compilation skipped (protoc unavailable): {e}"),
    }
}
