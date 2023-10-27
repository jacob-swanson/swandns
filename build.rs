fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src/proto/") // you can change the generated code's location
        .compile(&["proto/swandns.proto"], &["proto/"])
        .unwrap();
    Ok(())
}
