use garp_common::{ParticipantConfig, ParticipantId, SyncDomainId, GarpResult, GarpError};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub participant_config: ParticipantConfig,
    pub database: DatabaseConfig,
    pub api: ApiConfig,
    pub sync_domains: Vec<SyncDomainConfig>,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout: u64,
    pub redis_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub rate_limit: RateLimitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub burst_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDomainConfig {
    pub domain_id: SyncDomainId,
    pub endpoint: String,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub private_key_file: String,
    pub encryption_enabled: bool,
    pub signature_required: bool,
    pub trusted_peers: Vec<String>,
}

impl Config {
    /// Load configuration from file with optional overrides
    pub fn load(
        config_path: &str,
        participant_id: Option<String>,
        database_url: Option<String>,
        api_port: u16,
    ) -> GarpResult<Self> {
        let mut config = if Path::new(config_path).exists() {
            let config_str = std::fs::read_to_string(config_path)
                .map_err(|e| GarpError::Config(format!("Failed to read config file: {}", e)))?;
            
            toml::from_str(&config_str)
                .map_err(|e| GarpError::Config(format!("Failed to parse config file: {}", e)))?
        } else {
            Self::default()
        };

        // Apply command line overrides
        if let Some(id) = participant_id {
            config.participant_config.participant_id = ParticipantId::new(&id);
        }

        if let Some(url) = database_url {
            config.participant_config.database_url = url.clone();
            config.database.url = url;
        }

        config.participant_config.api_port = api_port;
        config.api.port = api_port;

        // Load private key
        config.load_private_key()?;

        Ok(config)
    }

    /// Load private key from file
    fn load_private_key(&mut self) -> GarpResult<()> {
        let key_path = &self.security.private_key_file;
        
        if Path::new(key_path).exists() {
            let key_data = std::fs::read(key_path)
                .map_err(|e| GarpError::Config(format!("Failed to read private key: {}", e)))?;
            
            self.participant_config.private_key = key_data;
        } else {
            // Generate new private key
            let private_key = garp_common::crypto::utils::generate_random_key();
            
            // Save to file
            std::fs::write(key_path, &private_key)
                .map_err(|e| GarpError::Config(format!("Failed to save private key: {}", e)))?;
            
            self.participant_config.private_key = private_key;
        }

        Ok(())
    }

    /// Save configuration to file
    pub fn save(&self, config_path: &str) -> GarpResult<()> {
        let config_str = toml::to_string_pretty(self)
            .map_err(|e| GarpError::Config(format!("Failed to serialize config: {}", e)))?;
        
        std::fs::write(config_path, config_str)
            .map_err(|e| GarpError::Config(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> GarpResult<()> {
        if self.participant_config.participant_id.0.is_empty() {
            return Err(GarpError::Config("Participant ID cannot be empty".to_string()));
        }

        if self.database.url.is_empty() {
            return Err(GarpError::Config("Database URL cannot be empty".to_string()));
        }

        if self.api.port == 0 {
            return Err(GarpError::Config("API port must be greater than 0".to_string()));
        }

        if self.participant_config.private_key.is_empty() {
            return Err(GarpError::Config("Private key cannot be empty".to_string()));
        }

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            participant_config: ParticipantConfig {
                participant_id: ParticipantId::new("participant-1"),
                private_key: Vec::new(),
                sync_domains: vec![SyncDomainId::new("domain-1")],
                database_url: "postgresql://localhost/garp_participant".to_string(),
                api_port: 8080,
            },
            database: DatabaseConfig {
                url: "postgresql://localhost/garp_participant".to_string(),
                max_connections: 10,
                connection_timeout: 30,
                redis_url: Some("redis://localhost:6379".to_string()),
            },
            api: ApiConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                cors_origins: vec!["*".to_string()],
                rate_limit: RateLimitConfig {
                    requests_per_minute: 100,
                    burst_size: 10,
                },
            },
            sync_domains: vec![
                SyncDomainConfig {
                    domain_id: SyncDomainId::new("domain-1"),
                    endpoint: "http://localhost:8081".to_string(),
                    public_key: Vec::new(),
                }
            ],
            security: SecurityConfig {
                private_key_file: "participant.key".to_string(),
                encryption_enabled: true,
                signature_required: true,
                trusted_peers: Vec::new(),
            },
        }
    }
}