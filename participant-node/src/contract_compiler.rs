//! Smart contract compiler that compiles Rust code to WASM bytecode
use garp_common::{GarpResult, GarpError, ContractError};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use tracing::{info, debug, error};

/// Contract compiler for compiling Rust smart contracts to WASM
pub struct ContractCompiler {
    /// Path to the Rust toolchain
    rust_toolchain: String,
    /// Path to the cargo command
    cargo_path: String,
}

impl ContractCompiler {
    /// Create a new contract compiler
    pub fn new() -> Self {
        Self {
            rust_toolchain: "stable".to_string(),
            cargo_path: "cargo".to_string(),
        }
    }

    /// Compile a Rust smart contract to WASM bytecode
    pub fn compile_contract(&self, source_code: &str) -> GarpResult<Vec<u8>> {
        info!("Compiling smart contract to WASM");
        
        // Create a temporary directory for compilation
        let temp_dir = TempDir::new()
            .map_err(|e| ContractError::CompilationFailed(format!("Failed to create temp directory: {}", e)))?;
        
        let temp_path = temp_dir.path();
        debug!("Created temporary directory: {:?}", temp_path);
        
        // Write the source code to a file
        let src_dir = temp_path.join("src");
        std::fs::create_dir_all(&src_dir)
            .map_err(|e| ContractError::CompilationFailed(format!("Failed to create src directory: {}", e)))?;
        
        let lib_rs_path = src_dir.join("lib.rs");
        std::fs::write(&lib_rs_path, source_code)
            .map_err(|e| ContractError::CompilationFailed(format!("Failed to write source code: {}", e)))?;
        
        // Create Cargo.toml
        let cargo_toml_content = r#"[package]
name = "garp_smart_contract"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
"#;
        
        let cargo_toml_path = temp_path.join("Cargo.toml");
        std::fs::write(&cargo_toml_path, cargo_toml_content)
            .map_err(|e| ContractError::CompilationFailed(format!("Failed to write Cargo.toml: {}", e)))?;
        
        // Run cargo build with WASM target
        let output = Command::new(&self.cargo_path)
            .current_dir(temp_path)
            .arg("build")
            .arg("--target")
            .arg("wasm32-unknown-unknown")
            .arg("--release")
            .output()
            .map_err(|e| ContractError::CompilationFailed(format!("Failed to execute cargo: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Cargo build failed: {}", stderr);
            return Err(ContractError::CompilationFailed(format!("Compilation failed: {}", stderr)).into());
        }
        
        // Read the compiled WASM bytecode
        let wasm_path = temp_path
            .join("target")
            .join("wasm32-unknown-unknown")
            .join("release")
            .join("garp_smart_contract.wasm");
        
        let wasm_bytecode = std::fs::read(&wasm_path)
            .map_err(|e| ContractError::CompilationFailed(format!("Failed to read WASM file: {}", e)))?;
        
        info!("Successfully compiled smart contract to WASM ({} bytes)", wasm_bytecode.len());
        Ok(wasm_bytecode)
    }

    /// Compile a smart contract from a file
    pub fn compile_contract_from_file<P: AsRef<Path>>(&self, file_path: P) -> GarpResult<Vec<u8>> {
        let source_code = std::fs::read_to_string(file_path)
            .map_err(|e| ContractError::CompilationFailed(format!("Failed to read source file: {}", e)))?;
        
        self.compile_contract(&source_code)
    }

    /// Validate WASM bytecode
    pub fn validate_bytecode(&self, bytecode: &[u8]) -> GarpResult<()> {
        // Check magic number (0x00 0x61 0x73 0x6D)
        if bytecode.len() < 8 {
            return Err(ContractError::ValidationFailed("Invalid WASM bytecode: too short".to_string()).into());
        }

        let magic = &bytecode[0..4];
        if magic != [0x00, 0x61, 0x73, 0x6D] {
            return Err(ContractError::ValidationFailed("Invalid WASM magic number".to_string()).into());
        }

        // Check version (0x01 0x00 0x00 0x00)
        let version = &bytecode[4..8];
        if version != [0x01, 0x00, 0x00, 0x00] {
            return Err(ContractError::ValidationFailed("Unsupported WASM version".to_string()).into());
        }

        // Additional validation would be implemented here
        // - Check for forbidden instructions
        // - Validate memory limits
        // - Check import/export sections

        Ok(())
    }
}

impl Default for ContractCompiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_simple_contract() {
        let compiler = ContractCompiler::new();
        
        // Simple test contract
        let source_code = r#"
#[no_mangle]
pub extern "C" fn init() {
    // Initialization function
}

#[no_mangle]
pub extern "C" fn execute() -> i32 {
    42
}

#[no_mangle]
pub extern "C" fn query() -> i32 {
    123
}
"#;
        
        let result = compiler.compile_contract(source_code);
        assert!(result.is_ok());
        
        let bytecode = result.unwrap();
        assert!(!bytecode.is_empty());
        
        // Validate the bytecode
        let validation = compiler.validate_bytecode(&bytecode);
        assert!(validation.is_ok());
    }
}