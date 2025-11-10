use participant_node::{
    PrivacyEngine, ZKSystem, ProofSystemType, SecureExecutionEnvironment,
    PrivateStateManager, PrivateTransactionProcessor, PrivacyLevel,
    StateOperation, PrivateTransaction, DisclosurePolicy
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_multi_party_private_computation() {
        // Test privacy-preserving multi-party computation scenario
        let mut engines = Vec::new();
        
        // Create multiple privacy engines for different parties
        for i in 0..3 {
            let engine = PrivacyEngine::new().await.unwrap();
            engines.push(engine);
        }
        
        // Deploy shared computation contract
        let mpc_contract = r#"
            contract MultiPartyComputation {
                private inputs: map<address, u64>;
                private participant_count: u64;
                private computation_result: u64;
                
                function submit_input(value: u64) private {
                    require(!inputs.contains(msg.sender));
                    inputs[msg.sender] = value;
                    participant_count += 1;
                }
                
                function compute_average() private {
                    require(participant_count >= 3);
                    let sum = 0;
                    for (addr, value) in inputs {
                        sum += value;
                    }
                    computation_result = sum / participant_count;
                }
                
                function get_result() public view returns u64 {
                    return computation_result;
                }
            }
        "#;
        
        // Each party deploys the contract
        let mut contract_ids = Vec::new();
        for (i, engine) in engines.iter_mut().enumerate() {
            let contract_id = engine.deploy_contract(
                format!("MPC_Party_{}", i),
                mpc_contract.to_string(),
                PrivacyLevel::High
            ).await.unwrap();
            contract_ids.push(contract_id);
        }
        
        // Each party submits their private input
        let private_inputs = vec![100u64, 200u64, 300u64];
        for (i, (engine, contract_id)) in engines.iter_mut().zip(contract_ids.iter()).enumerate() {
            let mut input_data = HashMap::new();
            input_data.insert("function".to_string(), "submit_input".to_string());
            input_data.insert("value".to_string(), private_inputs[i].to_string());
            
            let result = engine.execute_private_contract(
                contract_id.clone(),
                input_data,
                PrivacyLevel::High
            ).await;
            assert!(result.is_ok());
        }
        
        // Compute the result (average should be 200)
        let mut compute_data = HashMap::new();
        compute_data.insert("function".to_string(), "compute_average".to_string());
        
        let compute_result = engines[0].execute_private_contract(
            contract_ids[0].clone(),
            compute_data,
            PrivacyLevel::High
        ).await;
        assert!(compute_result.is_ok());
        
        // Verify all parties can access the result
        for (engine, contract_id) in engines.iter_mut().zip(contract_ids.iter()) {
            let mut result_data = HashMap::new();
            result_data.insert("function".to_string(), "get_result".to_string());
            
            let result = engine.execute_private_contract(
                contract_id.clone(),
                result_data,
                PrivacyLevel::Medium // Result can be less private
            ).await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_privacy_preserving_auction() {
        // Test a complete privacy-preserving auction system
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        // Deploy auction contract
        let auction_contract = r#"
            contract PrivateAuction {
                private bids: map<address, u64>;
                private bidder_commitments: map<address, bytes32>;
                private auction_end_time: u64;
                private highest_bid: u64;
                private winner: address;
                
                function commit_bid(commitment: bytes32) private {
                    require(block.timestamp < auction_end_time);
                    bidder_commitments[msg.sender] = commitment;
                }
                
                function reveal_bid(bid_amount: u64, nonce: u64) private {
                    require(block.timestamp >= auction_end_time);
                    let commitment = hash(bid_amount, nonce, msg.sender);
                    require(bidder_commitments[msg.sender] == commitment);
                    
                    bids[msg.sender] = bid_amount;
                    if bid_amount > highest_bid {
                        highest_bid = bid_amount;
                        winner = msg.sender;
                    }
                }
                
                function get_winner() public view returns address {
                    return winner;
                }
            }
        "#;
        
        let contract_id = engine.deploy_contract(
            "PrivateAuction".to_string(),
            auction_contract.to_string(),
            PrivacyLevel::High
        ).await.unwrap();
        
        // Simulate multiple bidders committing bids
        let bidders = vec!["bidder1", "bidder2", "bidder3"];
        let bid_amounts = vec![1000u64, 1500u64, 1200u64];
        
        // Phase 1: Commit bids
        for (bidder, &amount) in bidders.iter().zip(bid_amounts.iter()) {
            // Generate commitment (simplified)
            let commitment = format!("commitment_{}_{}", bidder, amount);
            
            let mut commit_data = HashMap::new();
            commit_data.insert("function".to_string(), "commit_bid".to_string());
            commit_data.insert("commitment".to_string(), commitment);
            
            let result = engine.execute_private_contract(
                contract_id.clone(),
                commit_data,
                PrivacyLevel::High
            ).await;
            assert!(result.is_ok());
        }
        
        // Phase 2: Reveal bids (after auction end time)
        for (bidder, &amount) in bidders.iter().zip(bid_amounts.iter()) {
            let mut reveal_data = HashMap::new();
            reveal_data.insert("function".to_string(), "reveal_bid".to_string());
            reveal_data.insert("bid_amount".to_string(), amount.to_string());
            reveal_data.insert("nonce".to_string(), "12345".to_string());
            
            let result = engine.execute_private_contract(
                contract_id.clone(),
                reveal_data,
                PrivacyLevel::High
            ).await;
            assert!(result.is_ok());
        }
        
        // Phase 3: Get winner (should be bidder2 with 1500)
        let mut winner_data = HashMap::new();
        winner_data.insert("function".to_string(), "get_winner".to_string());
        
        let winner_result = engine.execute_private_contract(
            contract_id,
            winner_data,
            PrivacyLevel::Medium
        ).await;
        assert!(winner_result.is_ok());
    }

    #[tokio::test]
    async fn test_privacy_preserving_voting_system() {
        // Test a complete privacy-preserving voting system
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        // Deploy voting contract
        let voting_contract = r#"
            contract PrivateVoting {
                private votes: map<address, u64>;
                private vote_commitments: map<address, bytes32>;
                private candidate_votes: map<u64, u64>;
                private total_votes: u64;
                private voting_ended: bool;
                
                function commit_vote(commitment: bytes32) private {
                    require(!voting_ended);
                    require(!vote_commitments.contains(msg.sender));
                    vote_commitments[msg.sender] = commitment;
                }
                
                function reveal_vote(candidate: u64, nonce: u64) private {
                    require(voting_ended);
                    let commitment = hash(candidate, nonce, msg.sender);
                    require(vote_commitments[msg.sender] == commitment);
                    
                    votes[msg.sender] = candidate;
                    candidate_votes[candidate] += 1;
                    total_votes += 1;
                }
                
                function end_voting() public {
                    voting_ended = true;
                }
                
                function get_results() public view returns map<u64, u64> {
                    require(voting_ended);
                    return candidate_votes;
                }
            }
        "#;
        
        let contract_id = engine.deploy_contract(
            "PrivateVoting".to_string(),
            voting_contract.to_string(),
            PrivacyLevel::High
        ).await.unwrap();
        
        // Simulate voters committing votes
        let voters = vec!["voter1", "voter2", "voter3", "voter4", "voter5"];
        let candidate_choices = vec![1u64, 2u64, 1u64, 3u64, 2u64]; // Votes for candidates 1, 2, 3
        
        // Phase 1: Commit votes
        for (voter, &candidate) in voters.iter().zip(candidate_choices.iter()) {
            let commitment = format!("vote_commitment_{}_{}", voter, candidate);
            
            let mut commit_data = HashMap::new();
            commit_data.insert("function".to_string(), "commit_vote".to_string());
            commit_data.insert("commitment".to_string(), commitment);
            
            let result = engine.execute_private_contract(
                contract_id.clone(),
                commit_data,
                PrivacyLevel::High
            ).await;
            assert!(result.is_ok());
        }
        
        // End voting period
        let mut end_data = HashMap::new();
        end_data.insert("function".to_string(), "end_voting".to_string());
        
        let end_result = engine.execute_private_contract(
            contract_id.clone(),
            end_data,
            PrivacyLevel::Medium
        ).await;
        assert!(end_result.is_ok());
        
        // Phase 2: Reveal votes
        for (voter, &candidate) in voters.iter().zip(candidate_choices.iter()) {
            let mut reveal_data = HashMap::new();
            reveal_data.insert("function".to_string(), "reveal_vote".to_string());
            reveal_data.insert("candidate".to_string(), candidate.to_string());
            reveal_data.insert("nonce".to_string(), "54321".to_string());
            
            let result = engine.execute_private_contract(
                contract_id.clone(),
                reveal_data,
                PrivacyLevel::High
            ).await;
            assert!(result.is_ok());
        }
        
        // Get final results
        let mut results_data = HashMap::new();
        results_data.insert("function".to_string(), "get_results".to_string());
        
        let results = engine.execute_private_contract(
            contract_id,
            results_data,
            PrivacyLevel::Low // Results can be public
        ).await;
        assert!(results.is_ok());
    }

    #[tokio::test]
    async fn test_cross_chain_privacy_bridge() {
        // Test privacy-preserving cross-chain transactions
        let mut source_engine = PrivacyEngine::new().await.unwrap();
        let mut dest_engine = PrivacyEngine::new().await.unwrap();
        
        // Deploy bridge contracts on both chains
        let bridge_contract = r#"
            contract PrivacyBridge {
                private locked_funds: map<address, u64>;
                private pending_transfers: map<bytes32, TransferData>;
                
                struct TransferData {
                    sender: address,
                    amount: u64,
                    destination_chain: u64,
                    recipient: address,
                    commitment: bytes32
                }
                
                function lock_funds(amount: u64, dest_chain: u64, recipient: address) private {
                    locked_funds[msg.sender] += amount;
                    let transfer_id = hash(msg.sender, amount, dest_chain, recipient, block.timestamp);
                    let commitment = generate_commitment(amount, recipient);
                    
                    pending_transfers[transfer_id] = TransferData {
                        sender: msg.sender,
                        amount: amount,
                        destination_chain: dest_chain,
                        recipient: recipient,
                        commitment: commitment
                    };
                }
                
                function unlock_funds(transfer_id: bytes32, proof: ZKProof) private {
                    require(verify_cross_chain_proof(proof, transfer_id));
                    let transfer = pending_transfers[transfer_id];
                    locked_funds[transfer.sender] -= transfer.amount;
                    // Mint equivalent on destination chain
                }
            }
        "#;
        
        let source_contract = source_engine.deploy_contract(
            "SourceBridge".to_string(),
            bridge_contract.to_string(),
            PrivacyLevel::High
        ).await.unwrap();
        
        let dest_contract = dest_engine.deploy_contract(
            "DestBridge".to_string(),
            bridge_contract.to_string(),
            PrivacyLevel::High
        ).await.unwrap();
        
        // Initiate cross-chain transfer
        let mut lock_data = HashMap::new();
        lock_data.insert("function".to_string(), "lock_funds".to_string());
        lock_data.insert("amount".to_string(), "1000".to_string());
        lock_data.insert("dest_chain".to_string(), "2".to_string());
        lock_data.insert("recipient".to_string(), "dest_address".to_string());
        
        let lock_result = source_engine.execute_private_contract(
            source_contract,
            lock_data,
            PrivacyLevel::High
        ).await;
        assert!(lock_result.is_ok());
        
        // Generate ZK proof for cross-chain verification
        let proof_result = source_engine.generate_zk_proof(
            ProofSystemType::STARK,
            vec![1, 0, 0, 0], // Proof of locked funds
            vec![1000u64, 2u64] // Amount and destination chain
        ).await;
        assert!(proof_result.is_ok());
        
        // Complete transfer on destination chain
        let mut unlock_data = HashMap::new();
        unlock_data.insert("function".to_string(), "unlock_funds".to_string());
        unlock_data.insert("transfer_id".to_string(), "transfer_123".to_string());
        unlock_data.insert("proof".to_string(), "zk_proof_data".to_string());
        
        let unlock_result = dest_engine.execute_private_contract(
            dest_contract,
            unlock_data,
            PrivacyLevel::High
        ).await;
        assert!(unlock_result.is_ok());
    }

    #[tokio::test]
    async fn test_privacy_preserving_defi_protocol() {
        // Test a complete privacy-preserving DeFi lending protocol
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        // Deploy lending protocol contract
        let lending_contract = r#"
            contract PrivateLending {
                private user_deposits: map<address, u64>;
                private user_borrows: map<address, u64>;
                private total_liquidity: u64;
                private total_borrowed: u64;
                private interest_rate: u64;
                
                function deposit(amount: u64) private {
                    user_deposits[msg.sender] += amount;
                    total_liquidity += amount;
                }
                
                function borrow(amount: u64) private {
                    require(amount <= calculate_max_borrow(msg.sender));
                    user_borrows[msg.sender] += amount;
                    total_borrowed += amount;
                }
                
                function repay(amount: u64) private {
                    require(user_borrows[msg.sender] >= amount);
                    user_borrows[msg.sender] -= amount;
                    total_borrowed -= amount;
                }
                
                function withdraw(amount: u64) private {
                    require(user_deposits[msg.sender] >= amount);
                    require(total_liquidity - amount >= total_borrowed);
                    user_deposits[msg.sender] -= amount;
                    total_liquidity -= amount;
                }
                
                function calculate_max_borrow(user: address) private view returns u64 {
                    return user_deposits[user] * 75 / 100; // 75% LTV
                }
            }
        "#;
        
        let contract_id = engine.deploy_contract(
            "PrivateLending".to_string(),
            lending_contract.to_string(),
            PrivacyLevel::High
        ).await.unwrap();
        
        // User 1: Deposit funds
        let mut deposit_data = HashMap::new();
        deposit_data.insert("function".to_string(), "deposit".to_string());
        deposit_data.insert("amount".to_string(), "10000".to_string());
        
        let deposit_result = engine.execute_private_contract(
            contract_id.clone(),
            deposit_data,
            PrivacyLevel::High
        ).await;
        assert!(deposit_result.is_ok());
        
        // User 1: Borrow against deposit
        let mut borrow_data = HashMap::new();
        borrow_data.insert("function".to_string(), "borrow".to_string());
        borrow_data.insert("amount".to_string(), "7000".to_string()); // 70% of deposit
        
        let borrow_result = engine.execute_private_contract(
            contract_id.clone(),
            borrow_data,
            PrivacyLevel::High
        ).await;
        assert!(borrow_result.is_ok());
        
        // Generate ZK proof of solvency (without revealing exact amounts)
        let solvency_proof = engine.generate_zk_proof(
            ProofSystemType::Bulletproofs,
            vec![1, 1, 0, 0], // Proof that deposits > borrows
            vec![1u64] // Public: protocol is solvent
        ).await;
        assert!(solvency_proof.is_ok());
        
        // User 1: Repay loan
        let mut repay_data = HashMap::new();
        repay_data.insert("function".to_string(), "repay".to_string());
        repay_data.insert("amount".to_string(), "3000".to_string());
        
        let repay_result = engine.execute_private_contract(
            contract_id.clone(),
            repay_data,
            PrivacyLevel::High
        ).await;
        assert!(repay_result.is_ok());
        
        // User 1: Withdraw remaining deposit
        let mut withdraw_data = HashMap::new();
        withdraw_data.insert("function".to_string(), "withdraw".to_string());
        withdraw_data.insert("amount".to_string(), "5000".to_string());
        
        let withdraw_result = engine.execute_private_contract(
            contract_id,
            withdraw_data,
            PrivacyLevel::High
        ).await;
        assert!(withdraw_result.is_ok());
    }

    #[tokio::test]
    async fn test_performance_under_load() {
        // Test privacy engine performance under concurrent load
        let engine = PrivacyEngine::new().await.unwrap();
        
        let num_concurrent_operations = 10;
        let mut handles = Vec::new();
        
        // Launch concurrent privacy operations
        for i in 0..num_concurrent_operations {
            let mut engine_clone = engine.clone();
            let handle = tokio::spawn(async move {
                // Deploy contract
                let contract_code = format!(r#"
                    contract LoadTestContract{} {{
                        private counter: u64;
                        
                        function increment() private {{
                            counter += 1;
                        }}
                        
                        function get_count() private view returns u64 {{
                            return counter;
                        }}
                    }}
                "#, i);
                
                let contract_id = engine_clone.deploy_contract(
                    format!("LoadTest{}", i),
                    contract_code,
                    PrivacyLevel::Medium
                ).await?;
                
                // Execute multiple operations
                for j in 0..5 {
                    let mut call_data = HashMap::new();
                    call_data.insert("function".to_string(), "increment".to_string());
                    
                    engine_clone.execute_private_contract(
                        contract_id.clone(),
                        call_data,
                        PrivacyLevel::Medium
                    ).await?;
                }
                
                // Generate ZK proof
                engine_clone.generate_zk_proof(
                    ProofSystemType::Groth16,
                    vec![i as u8; 32],
                    vec![i as u64]
                ).await?;
                
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete with timeout
        let results = timeout(
            Duration::from_secs(30),
            futures::future::join_all(handles)
        ).await;
        
        assert!(results.is_ok(), "Operations should complete within timeout");
        
        let results = results.unwrap();
        for result in results {
            assert!(result.is_ok(), "All concurrent operations should succeed");
            assert!(result.unwrap().is_ok(), "All privacy operations should succeed");
        }
        
        // Verify final metrics
        let final_metrics = engine.get_metrics().await.unwrap();
        assert!(final_metrics.contracts_deployed >= num_concurrent_operations as u64);
        assert!(final_metrics.contracts_executed >= (num_concurrent_operations * 5) as u64);
        assert!(final_metrics.zk_proofs_generated >= num_concurrent_operations as u64);
    }

    #[tokio::test]
    async fn test_privacy_engine_recovery_and_resilience() {
        // Test privacy engine recovery from various failure scenarios
        let mut engine = PrivacyEngine::new().await.unwrap();
        
        // Test recovery from invalid contract execution
        let invalid_contract = r#"
            contract BuggyContract {
                private value: u64;
                
                function divide_by_zero() private {
                    value = 100 / 0; // This should fail
                }
            }
        "#;
        
        let contract_id = engine.deploy_contract(
            "BuggyContract".to_string(),
            invalid_contract.to_string(),
            PrivacyLevel::Medium
        ).await.unwrap();
        
        // Execute failing function
        let mut fail_data = HashMap::new();
        fail_data.insert("function".to_string(), "divide_by_zero".to_string());
        
        let fail_result = engine.execute_private_contract(
            contract_id,
            fail_data,
            PrivacyLevel::Medium
        ).await;
        
        // Should handle error gracefully
        assert!(fail_result.is_err());
        
        // Engine should still be functional after error
        let valid_contract = r#"
            contract ValidContract {
                private value: u64;
                
                function set_value(v: u64) private {
                    value = v;
                }
            }
        "#;
        
        let valid_contract_id = engine.deploy_contract(
            "ValidContract".to_string(),
            valid_contract.to_string(),
            PrivacyLevel::Medium
        ).await;
        
        assert!(valid_contract_id.is_ok(), "Engine should recover and work normally");
        
        // Verify metrics show both success and failure
        let metrics = engine.get_metrics().await.unwrap();
        assert!(metrics.contracts_deployed >= 2);
        assert!(metrics.execution_errors > 0);
    }
}