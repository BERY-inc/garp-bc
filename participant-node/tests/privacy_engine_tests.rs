use participant_node::{
    PrivacyEngine, PrivacyDSLParser, PrivacyDSLCompiler, PrivacyContract,
    ZKSystem, ProofSystemType, SecureExecutionEnvironment, IsolationLevel,
    PrivateStateManager, StateOperation, PrivacyLevel, StateOperationResult,
    PrivateTransactionProcessor, PrivateTransaction, DisclosurePolicy
};
use std::collections::HashMap;
use tokio;

#[cfg(test)]
mod privacy_engine_tests {
    use super::*;

    #[tokio::test]
    async fn test_privacy_engine_initialization() {
        let engine = PrivacyEngine::new().await;
        assert!(engine.is_ok());
        
        let engine = engine.unwrap();
        assert!(engine.is_initialized().await);
        
        let metrics = engine.get_metrics().await;
        assert!(metrics.is_ok());
    }

    #[tokio::test]
    async fn test_privacy_engine_contract_deployment() {
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        let contract_code = r#"
            contract PrivateVoting {
                private votes: map<address, bool>;
                public total_votes: u64;
                
                function vote(choice: bool) private {
                    require(!votes.contains(msg.sender));
                    votes[msg.sender] = choice;
                    if choice {
                        total_votes += 1;
                    }
                }
                
                function get_total() public view returns u64 {
                    return total_votes;
                }
            }
        "#;
        
        let deployment_result = engine.deploy_contract(
            "PrivateVoting".to_string(),
            contract_code.to_string(),
            PrivacyLevel::High
        ).await;
        
        assert!(deployment_result.is_ok());
        let contract_id = deployment_result.unwrap();
        assert!(!contract_id.is_empty());
        
        // Verify contract is deployed
        let contract_info = engine.get_contract_info(&contract_id).await;
        assert!(contract_info.is_ok());
    }

    #[tokio::test]
    async fn test_privacy_engine_private_execution() {
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        // Deploy a simple private contract
        let contract_code = r#"
            contract PrivateCounter {
                private counter: u64;
                
                function increment() private {
                    counter += 1;
                }
                
                function get_count() private view returns u64 {
                    return counter;
                }
            }
        "#;
        
        let contract_id = engine.deploy_contract(
            "PrivateCounter".to_string(),
            contract_code.to_string(),
            PrivacyLevel::High
        ).await.unwrap();
        
        // Execute private function
        let mut call_data = HashMap::new();
        call_data.insert("function".to_string(), "increment".to_string());
        
        let execution_result = engine.execute_private_contract(
            contract_id.clone(),
            call_data,
            PrivacyLevel::High
        ).await;
        
        assert!(execution_result.is_ok());
        
        // Verify state change occurred (through metrics or other means)
        let metrics = engine.get_metrics().await.unwrap();
        assert!(metrics.contracts_executed > 0);
    }

    #[tokio::test]
    async fn test_privacy_engine_zk_proof_generation() {
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        let proof_data = vec![1u8, 2, 3, 4, 5];
        let public_inputs = vec![42u64, 100];
        
        let proof_result = engine.generate_zk_proof(
            ProofSystemType::Groth16,
            proof_data,
            public_inputs
        ).await;
        
        assert!(proof_result.is_ok());
        let proof = proof_result.unwrap();
        
        // Verify proof structure
        assert!(!proof.proof_data.is_empty());
        assert!(proof.verification_key.is_some());
    }

    #[tokio::test]
    async fn test_privacy_engine_selective_disclosure() {
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        // Create a transaction with selective disclosure
        let transaction_data = HashMap::from([
            ("amount".to_string(), "1000".to_string()),
            ("recipient".to_string(), "0x123...".to_string()),
            ("memo".to_string(), "Private payment".to_string()),
        ]);
        
        let disclosure_policy = DisclosurePolicy::new(vec![
            ("amount".to_string(), true),  // Disclose amount
            ("recipient".to_string(), false), // Keep recipient private
            ("memo".to_string(), false),   // Keep memo private
        ]);
        
        let disclosure_result = engine.create_selective_disclosure(
            transaction_data,
            disclosure_policy
        ).await;
        
        assert!(disclosure_result.is_ok());
        let disclosure = disclosure_result.unwrap();
        
        // Verify selective disclosure properties
        assert!(disclosure.disclosed_fields.contains_key("amount"));
        assert!(!disclosure.disclosed_fields.contains_key("recipient"));
        assert!(!disclosure.disclosed_fields.contains_key("memo"));
        assert!(!disclosure.commitment.is_empty());
    }

    #[tokio::test]
    async fn test_privacy_engine_state_management() {
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        let state_key = "test_private_state".to_string();
        let state_value = vec![1u8, 2, 3, 4, 5];
        
        // Create private state
        let create_result = engine.create_private_state(
            state_key.clone(),
            state_value.clone(),
            PrivacyLevel::High
        ).await;
        
        assert!(create_result.is_ok());
        
        // Read private state
        let read_result = engine.read_private_state(
            state_key.clone(),
            PrivacyLevel::High
        ).await;
        
        assert!(read_result.is_ok());
        let retrieved_value = read_result.unwrap();
        assert_eq!(retrieved_value, state_value);
        
        // Update private state
        let new_value = vec![6u8, 7, 8, 9, 10];
        let update_result = engine.update_private_state(
            state_key.clone(),
            new_value.clone(),
            PrivacyLevel::High
        ).await;
        
        assert!(update_result.is_ok());
        
        // Verify update
        let updated_value = engine.read_private_state(
            state_key.clone(),
            PrivacyLevel::High
        ).await.unwrap();
        assert_eq!(updated_value, new_value);
    }

    #[tokio::test]
    async fn test_privacy_engine_transaction_processing() {
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        // Create a private transaction
        let transaction = PrivateTransaction::new(
            "sender_address".to_string(),
            "recipient_address".to_string(),
            1000u64,
            PrivacyLevel::High,
            Some("Private transfer".to_string())
        );
        
        let processing_result = engine.process_private_transaction(transaction).await;
        assert!(processing_result.is_ok());
        
        let transaction_id = processing_result.unwrap();
        assert!(!transaction_id.is_empty());
        
        // Verify transaction was processed
        let transaction_status = engine.get_transaction_status(&transaction_id).await;
        assert!(transaction_status.is_ok());
    }

    #[tokio::test]
    async fn test_privacy_engine_error_handling() {
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        // Test invalid contract deployment
        let invalid_contract = "invalid contract syntax {{{";
        let deployment_result = engine.deploy_contract(
            "InvalidContract".to_string(),
            invalid_contract.to_string(),
            PrivacyLevel::High
        ).await;
        
        assert!(deployment_result.is_err());
        
        // Test reading non-existent state
        let read_result = engine.read_private_state(
            "non_existent_key".to_string(),
            PrivacyLevel::High
        ).await;
        
        assert!(read_result.is_err());
        
        // Test invalid ZK proof generation
        let invalid_proof_result = engine.generate_zk_proof(
            ProofSystemType::Groth16,
            vec![], // Empty proof data
            vec![]  // Empty public inputs
        ).await;
        
        assert!(invalid_proof_result.is_err());
    }

    #[tokio::test]
    async fn test_privacy_engine_performance_metrics() {
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        // Perform several operations to generate metrics
        for i in 0..5 {
            let contract_code = format!(r#"
                contract TestContract{} {{
                    private value: u64;
                    
                    function set_value(v: u64) private {{
                        value = v;
                    }}
                }}
            "#, i);
            
            let _ = engine.deploy_contract(
                format!("TestContract{}", i),
                contract_code,
                PrivacyLevel::Medium
            ).await;
        }
        
        let metrics = engine.get_metrics().await.unwrap();
        assert!(metrics.contracts_deployed >= 5);
        assert!(metrics.total_execution_time > 0);
        assert!(metrics.average_execution_time > 0);
    }

    #[tokio::test]
    async fn test_privacy_engine_concurrent_operations() {
        let engine = PrivacyEngine::new().await.unwrap();
        
        // Test concurrent contract deployments
        let mut handles = vec![];
        
        for i in 0..3 {
            let mut engine_clone = engine.clone();
            let handle = tokio::spawn(async move {
                let contract_code = format!(r#"
                    contract ConcurrentContract{} {{
                        private data: u64;
                        
                        function store(value: u64) private {{
                            data = value;
                        }}
                    }}
                "#, i);
                
                engine_clone.deploy_contract(
                    format!("ConcurrentContract{}", i),
                    contract_code,
                    PrivacyLevel::Medium
                ).await
            });
            handles.push(handle);
        }
        
        // Wait for all deployments to complete
        let results: Vec<_> = futures::future::join_all(handles).await;
        
        // Verify all deployments succeeded
        for result in results {
            assert!(result.is_ok());
            assert!(result.unwrap().is_ok());
        }
    }
}

#[cfg(test)]
mod component_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_dsl_to_zk_integration() {
        // Test DSL parsing and compilation with ZK proof generation
        let dsl_code = r#"
            contract PrivateAuction {
                private bids: map<address, u64>;
                private highest_bid: u64;
                
                function place_bid(amount: u64) private {
                    require(amount > highest_bid);
                    bids[msg.sender] = amount;
                    highest_bid = amount;
                }
                
                function reveal_winner() public returns address {
                    // ZK proof that winner has highest bid without revealing amounts
                    return find_highest_bidder();
                }
            }
        "#;
        
        let parser = PrivacyDSLParser::new();
        let parse_result = parser.parse(dsl_code);
        assert!(parse_result.is_ok());
        
        let ast = parse_result.unwrap();
        let compiler = PrivacyDSLCompiler::new();
        let compile_result = compiler.compile(ast);
        assert!(compile_result.is_ok());
        
        let contract = compile_result.unwrap();
        
        // Generate ZK circuit for the contract
        let zk_system = ZKSystem::new();
        let circuit_result = zk_system.compile_contract_circuit(&contract).await;
        assert!(circuit_result.is_ok());
    }

    #[tokio::test]
    async fn test_state_to_execution_integration() {
        // Test private state management with secure execution
        let state_manager = PrivateStateManager::new().await.unwrap();
        let execution_env = SecureExecutionEnvironment::new(IsolationLevel::High).await.unwrap();
        
        // Create private state
        let state_key = "integration_test_state".to_string();
        let initial_value = vec![42u8; 32];
        
        let create_op = StateOperation::Create {
            key: state_key.clone(),
            value: initial_value.clone(),
            privacy_level: PrivacyLevel::High,
        };
        
        let create_result = state_manager.execute_operation(create_op).await;
        assert!(create_result.is_ok());
        
        // Execute contract that modifies the state
        let contract_code = r#"
            function modify_state() {
                let current = read_state("integration_test_state");
                let modified = transform(current);
                write_state("integration_test_state", modified);
            }
        "#;
        
        let execution_result = execution_env.execute_contract(
            "test_contract".to_string(),
            contract_code.to_string(),
            HashMap::new()
        ).await;
        
        assert!(execution_result.is_ok());
    }

    #[tokio::test]
    async fn test_transaction_to_disclosure_integration() {
        // Test private transaction processing with selective disclosure
        let transaction_processor = PrivateTransactionProcessor::new().await.unwrap();
        
        // Create transaction with disclosure policy
        let transaction = PrivateTransaction::new(
            "sender123".to_string(),
            "recipient456".to_string(),
            5000u64,
            PrivacyLevel::High,
            Some("Integration test payment".to_string())
        );
        
        let disclosure_policy = DisclosurePolicy::new(vec![
            ("amount".to_string(), true),     // Reveal amount for compliance
            ("sender".to_string(), false),    // Keep sender private
            ("recipient".to_string(), false), // Keep recipient private
            ("memo".to_string(), false),      // Keep memo private
        ]);
        
        // Submit transaction
        let submit_result = transaction_processor.submit_transaction(transaction).await;
        assert!(submit_result.is_ok());
        
        let transaction_id = submit_result.unwrap();
        
        // Request selective disclosure
        let disclosure_result = transaction_processor.request_disclosure(
            transaction_id,
            disclosure_policy,
            "compliance_authority".to_string()
        ).await;
        
        assert!(disclosure_result.is_ok());
        let disclosure = disclosure_result.unwrap();
        
        // Verify only amount is disclosed
        assert!(disclosure.disclosed_fields.contains_key("amount"));
        assert_eq!(disclosure.disclosed_fields.len(), 1);
    }

    #[tokio::test]
    async fn test_end_to_end_privacy_workflow() {
        // Complete end-to-end test of privacy-preserving workflow
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        // 1. Deploy privacy contract
        let contract_code = r#"
            contract PrivateEscrow {
                private deposits: map<address, u64>;
                private locked_funds: u64;
                
                function deposit(amount: u64) private {
                    deposits[msg.sender] += amount;
                    locked_funds += amount;
                }
                
                function withdraw(amount: u64) private {
                    require(deposits[msg.sender] >= amount);
                    deposits[msg.sender] -= amount;
                    locked_funds -= amount;
                }
                
                function get_balance() private view returns u64 {
                    return deposits[msg.sender];
                }
            }
        "#;
        
        let contract_id = engine.deploy_contract(
            "PrivateEscrow".to_string(),
            contract_code.to_string(),
            PrivacyLevel::High
        ).await.unwrap();
        
        // 2. Execute private deposit
        let mut deposit_data = HashMap::new();
        deposit_data.insert("function".to_string(), "deposit".to_string());
        deposit_data.insert("amount".to_string(), "1000".to_string());
        
        let deposit_result = engine.execute_private_contract(
            contract_id.clone(),
            deposit_data,
            PrivacyLevel::High
        ).await;
        assert!(deposit_result.is_ok());
        
        // 3. Generate ZK proof of deposit
        let proof_result = engine.generate_zk_proof(
            ProofSystemType::PLONK,
            vec![1, 0, 0, 0], // Proof that deposit occurred
            vec![1000u64]     // Public: amount deposited
        ).await;
        assert!(proof_result.is_ok());
        
        // 4. Create private transaction for withdrawal
        let withdrawal_tx = PrivateTransaction::new(
            "user_address".to_string(),
            contract_id.clone(),
            500u64, // Withdraw 500
            PrivacyLevel::High,
            Some("Escrow withdrawal".to_string())
        );
        
        let tx_result = engine.process_private_transaction(withdrawal_tx).await;
        assert!(tx_result.is_ok());
        
        // 5. Verify final state through selective disclosure
        let balance_data = HashMap::from([
            ("function".to_string(), "get_balance".to_string()),
        ]);
        
        let balance_result = engine.execute_private_contract(
            contract_id,
            balance_data,
            PrivacyLevel::High
        ).await;
        assert!(balance_result.is_ok());
        
        // 6. Check final metrics
        let final_metrics = engine.get_metrics().await.unwrap();
        assert!(final_metrics.contracts_deployed >= 1);
        assert!(final_metrics.contracts_executed >= 2);
        assert!(final_metrics.zk_proofs_generated >= 1);
        assert!(final_metrics.transactions_processed >= 1);
    }
}