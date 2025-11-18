use crate::bridge::wallet::{WalletManager, CreateWalletRequest};

#[tokio::test]
async fn test_wallet_manager_creation() {
    let wallet_manager = WalletManager::new();
    assert!(wallet_manager.list_wallets().await.is_empty());
}

#[tokio::test]
async fn test_wallet_creation_and_retrieval() {
    let wallet_manager = WalletManager::new();
    
    let request = CreateWalletRequest {
        chain_type: "ethereum".to_string(),
        password: "test_password".to_string(),
    };
    
    let response = wallet_manager.create_wallet(request).await.unwrap();
    assert!(!response.wallet_id.is_empty());
    assert!(!response.address.is_empty());
    
    let wallets = wallet_manager.list_wallets().await;
    assert_eq!(wallets.len(), 1);
    
    let retrieved_wallet = wallet_manager.get_wallet(&response.wallet_id).await;
    assert!(retrieved_wallet.is_some());
    assert_eq!(retrieved_wallet.unwrap().address, response.address);
}

#[tokio::test]
async fn test_multiple_wallet_creation() {
    let wallet_manager = WalletManager::new();
    
    // Create Ethereum wallet
    let eth_request = CreateWalletRequest {
        chain_type: "ethereum".to_string(),
        password: "eth_password".to_string(),
    };
    let eth_response = wallet_manager.create_wallet(eth_request).await.unwrap();
    
    // Create Solana wallet
    let sol_request = CreateWalletRequest {
        chain_type: "solana".to_string(),
        password: "sol_password".to_string(),
    };
    let sol_response = wallet_manager.create_wallet(sol_request).await.unwrap();
    
    // Verify both wallets exist
    let wallets = wallet_manager.list_wallets().await;
    assert_eq!(wallets.len(), 2);
    
    assert_ne!(eth_response.address, sol_response.address);
}

#[tokio::test]
async fn test_private_key_decryption() {
    let wallet_manager = WalletManager::new();
    
    let request = CreateWalletRequest {
        chain_type: "ethereum".to_string(),
        password: "test_password".to_string(),
    };
    
    let response = wallet_manager.create_wallet(request).await.unwrap();
    
    let decrypted_key = wallet_manager.decrypt_private_key(&response.wallet_id, "test_password").await;
    assert!(decrypted_key.is_ok());
    assert!(!decrypted_key.unwrap().is_empty());
}