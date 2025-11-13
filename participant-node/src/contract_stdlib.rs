//! Standard library for GARP smart contracts
//!
//! This module provides a set of standard functions and utilities that can be used
//! by smart contracts running on the GARP blockchain.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Standard library context for smart contracts
pub struct StdlibContext {
    /// Contract storage
    pub storage: HashMap<String, Value>,
    /// Contract caller
    pub caller: String,
    /// Current block timestamp
    pub timestamp: i64,
    /// Contract balance
    pub balance: u64,
}

impl StdlibContext {
    /// Create a new standard library context
    pub fn new(caller: String, timestamp: i64, balance: u64) -> Self {
        Self {
            storage: HashMap::new(),
            caller,
            timestamp,
            balance,
        }
    }
}

/// Result type for standard library functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StdlibResult {
    /// Whether the operation was successful
    pub success: bool,
    /// Return value (if any)
    pub value: Option<Value>,
    /// Error message (if any)
    pub error: Option<String>,
}

impl StdlibResult {
    /// Create a successful result with a value
    pub fn success(value: Value) -> Self {
        Self {
            success: true,
            value: Some(value),
            error: None,
        }
    }

    /// Create a successful result with no value
    pub fn success_empty() -> Self {
        Self {
            success: true,
            value: None,
            error: None,
        }
    }

    /// Create an error result
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            value: None,
            error: Some(message),
        }
    }
}

/// Storage operations
pub mod storage {
    use super::*;

    /// Get a value from contract storage
    pub fn get(ctx: &StdlibContext, key: &str) -> StdlibResult {
        match ctx.storage.get(key) {
            Some(value) => StdlibResult::success(value.clone()),
            None => StdlibResult::success(Value::Null),
        }
    }

    /// Set a value in contract storage
    pub fn set(ctx: &mut StdlibContext, key: String, value: Value) -> StdlibResult {
        ctx.storage.insert(key, value);
        StdlibResult::success_empty()
    }

    /// Delete a value from contract storage
    pub fn delete(ctx: &mut StdlibContext, key: &str) -> StdlibResult {
        ctx.storage.remove(key);
        StdlibResult::success_empty()
    }

    /// Check if a key exists in contract storage
    pub fn exists(ctx: &StdlibContext, key: &str) -> StdlibResult {
        let exists = ctx.storage.contains_key(key);
        StdlibResult::success(Value::Bool(exists))
    }
}

/// Math operations
pub mod math {
    use super::*;

    /// Add two numbers
    pub fn add(a: i64, b: i64) -> StdlibResult {
        let result = a.checked_add(b);
        match result {
            Some(value) => StdlibResult::success(Value::Number(value.into())),
            None => StdlibResult::error("Integer overflow".to_string()),
        }
    }

    /// Subtract two numbers
    pub fn sub(a: i64, b: i64) -> StdlibResult {
        let result = a.checked_sub(b);
        match result {
            Some(value) => StdlibResult::success(Value::Number(value.into())),
            None => StdlibResult::error("Integer underflow".to_string()),
        }
    }

    /// Multiply two numbers
    pub fn mul(a: i64, b: i64) -> StdlibResult {
        let result = a.checked_mul(b);
        match result {
            Some(value) => StdlibResult::success(Value::Number(value.into())),
            None => StdlibResult::error("Integer overflow".to_string()),
        }
    }

    /// Divide two numbers
    pub fn div(a: i64, b: i64) -> StdlibResult {
        if b == 0 {
            return StdlibResult::error("Division by zero".to_string());
        }
        let result = a.checked_div(b);
        match result {
            Some(value) => StdlibResult::success(Value::Number(value.into())),
            None => StdlibResult::error("Division error".to_string()),
        }
    }

    /// Calculate the power of a number
    pub fn pow(base: i64, exp: u32) -> StdlibResult {
        let result = base.checked_pow(exp);
        match result {
            Some(value) => StdlibResult::success(Value::Number(value.into())),
            None => StdlibResult::error("Power calculation overflow".to_string()),
        }
    }
}

/// String operations
pub mod string {
    use super::*;
    use base64::{encode, decode};

    /// Concatenate two strings
    pub fn concat(a: &str, b: &str) -> StdlibResult {
        let result = format!("{}{}", a, b);
        StdlibResult::success(Value::String(result))
    }

    /// Get the length of a string
    pub fn len(s: &str) -> StdlibResult {
        StdlibResult::success(Value::Number(s.len().into()))
    }

    /// Convert string to uppercase
    pub fn to_uppercase(s: &str) -> StdlibResult {
        let result = s.to_uppercase();
        StdlibResult::success(Value::String(result))
    }

    /// Convert string to lowercase
    pub fn to_lowercase(s: &str) -> StdlibResult {
        let result = s.to_lowercase();
        StdlibResult::success(Value::String(result))
    }

    /// Encode string to base64
    pub fn to_base64(s: &str) -> StdlibResult {
        let result = encode(s);
        StdlibResult::success(Value::String(result))
    }

    /// Decode string from base64
    pub fn from_base64(s: &str) -> StdlibResult {
        match decode(s) {
            Ok(decoded) => match String::from_utf8(decoded) {
                Ok(result) => StdlibResult::success(Value::String(result)),
                Err(_) => StdlibResult::error("Invalid UTF-8 sequence".to_string()),
            },
            Err(_) => StdlibResult::error("Invalid base64 string".to_string()),
        }
    }
}

/// Cryptographic operations
pub mod crypto {
    use super::*;
    use sha2::{Sha256, Digest};
    use ring::signature;

    /// Hash data using SHA-256
    pub fn sha256(data: &[u8]) -> StdlibResult {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let hex_result = format!("{:x}", result);
        StdlibResult::success(Value::String(hex_result))
    }

    /// Verify an Ed25519 signature
    pub fn verify_ed25519_signature(
        message: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> StdlibResult {
        let peer_public_key = match signature::UnparsedPublicKey::new(
            &signature::ED25519,
            public_key,
        ) {
            Ok(key) => key,
            Err(_) => return StdlibResult::error("Invalid public key".to_string()),
        };

        let result = peer_public_key.verify(message, signature);
        StdlibResult::success(Value::Bool(result.is_ok()))
    }
}

/// Utility functions
pub mod utils {
    use super::*;
    use chrono::{DateTime, Utc};

    /// Get current timestamp
    pub fn get_timestamp(ctx: &StdlibContext) -> StdlibResult {
        StdlibResult::success(Value::Number(ctx.timestamp.into()))
    }

    /// Get caller address
    pub fn get_caller(ctx: &StdlibContext) -> StdlibResult {
        StdlibResult::success(Value::String(ctx.caller.clone()))
    }

    /// Get contract balance
    pub fn get_balance(ctx: &StdlibContext) -> StdlibResult {
        StdlibResult::success(Value::Number(ctx.balance.into()))
    }

    /// Format timestamp as ISO string
    pub fn format_timestamp(timestamp: i64) -> StdlibResult {
        let dt = DateTime::<Utc>::from_timestamp(timestamp, 0)
            .unwrap_or_else(|| Utc::now());
        let formatted = dt.to_rfc3339();
        StdlibResult::success(Value::String(formatted))
    }
}

/// Event emission
pub mod events {
    use super::*;
    use std::collections::HashMap;

    /// Emit an event
    pub fn emit(ctx: &mut StdlibContext, event_name: String, data: Value) -> StdlibResult {
        // In a real implementation, this would emit an event that can be indexed
        // For now, we'll just store it in a special storage key
        let events_key = "_events";
        let mut events = match ctx.storage.get(events_key) {
            Some(Value::Array(arr)) => arr.clone(),
            _ => Vec::new(),
        };
        
        let event = serde_json::json!({
            "name": event_name,
            "data": data,
            "timestamp": ctx.timestamp
        });
        
        events.push(event);
        ctx.storage.insert(events_key.to_string(), Value::Array(events));
        
        StdlibResult::success_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_operations() {
        let mut ctx = StdlibContext::new("caller1".to_string(), 1234567890, 1000);
        
        // Test set and get
        let result = storage::set(&mut ctx, "test_key".to_string(), Value::String("test_value".to_string()));
        assert!(result.success);
        
        let result = storage::get(&ctx, "test_key");
        assert!(result.success);
        assert_eq!(result.value, Some(Value::String("test_value".to_string())));
        
        // Test exists
        let result = storage::exists(&ctx, "test_key");
        assert!(result.success);
        assert_eq!(result.value, Some(Value::Bool(true)));
        
        // Test delete
        let result = storage::delete(&mut ctx, "test_key");
        assert!(result.success);
        
        let result = storage::get(&ctx, "test_key");
        assert!(result.success);
        assert_eq!(result.value, Some(Value::Null));
    }

    #[test]
    fn test_math_operations() {
        // Test addition
        let result = math::add(5, 3);
        assert!(result.success);
        assert_eq!(result.value, Some(Value::Number(8.into())));
        
        // Test subtraction
        let result = math::sub(5, 3);
        assert!(result.success);
        assert_eq!(result.value, Some(Value::Number(2.into())));
        
        // Test multiplication
        let result = math::mul(5, 3);
        assert!(result.success);
        assert_eq!(result.value, Some(Value::Number(15.into())));
        
        // Test division
        let result = math::div(6, 3);
        assert!(result.success);
        assert_eq!(result.value, Some(Value::Number(2.into())));
        
        // Test power
        let result = math::pow(2, 3);
        assert!(result.success);
        assert_eq!(result.value, Some(Value::Number(8.into())));
    }

    #[test]
    fn test_string_operations() {
        // Test concatenation
        let result = string::concat("hello", " world");
        assert!(result.success);
        assert_eq!(result.value, Some(Value::String("hello world".to_string())));
        
        // Test length
        let result = string::len("hello");
        assert!(result.success);
        assert_eq!(result.value, Some(Value::Number(5.into())));
        
        // Test uppercase
        let result = string::to_uppercase("hello");
        assert!(result.success);
        assert_eq!(result.value, Some(Value::String("HELLO".to_string())));
        
        // Test base64 encoding/decoding
        let result = string::to_base64("hello");
        assert!(result.success);
        let encoded = result.value.clone().unwrap().as_str().unwrap().to_string();
        
        let result = string::from_base64(&encoded);
        assert!(result.success);
        assert_eq!(result.value, Some(Value::String("hello".to_string())));
    }
}