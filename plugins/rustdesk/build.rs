fn main() {
    // Compile protobuf files for RustDesk protocol

    compile_protos();

    // Rerun if the script itself changes
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=protos/rendezvous.proto");
    println!("cargo:rerun-if-changed=protos/message.proto");
}

/// Compile protobuf files using protobuf-codegen (same as RustDesk server)

fn compile_protos() {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let protos_dir = out_dir.join("protos");
    std::fs::create_dir_all(&protos_dir).unwrap();

    protobuf_codegen::Codegen::new()
        .pure()
        .out_dir(&protos_dir)
        .inputs(["protos/rendezvous.proto", "protos/message.proto"])
        .include("protos")
        .customize(protobuf_codegen::Customize::default().tokio_bytes(true))
        .run()
        .expect("Failed to compile protobuf files");

    // Generate mod.rs for the protos module
    let mod_content = r#"pub mod rendezvous;
pub mod message;
"#;
    std::fs::write(protos_dir.join("mod.rs"), mod_content).unwrap();
}
