fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile the protobuf files
    tonic_build::compile_protos("proto/garp.proto")?;
    
    Ok(())
}