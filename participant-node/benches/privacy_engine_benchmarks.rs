use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use participant_node::{
    PrivacyEngine, ZKSystem, ProofSystemType, SecureExecutionEnvironment,
    PrivateStateManager, PrivateTransactionProcessor, PrivacyLevel,
    StateOperation, PrivateTransaction, DisclosurePolicy
};
use std::collections::HashMap;
use tokio::runtime::Runtime;

fn bench_privacy_engine_initialization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("privacy_engine_init", |b| {
        b.to_async(&rt).iter(|| async {
            let engine = PrivacyEngine::new().await;
            black_box(engine)
        })
    });
}

fn bench_contract_deployment(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("contract_deployment");
    
    let contract_sizes = vec![
        ("small", r#"
            contract SmallContract {
                private value: u64;
                function set(v: u64) private { value = v; }
            }
        "#),
        ("medium", r#"
            contract MediumContract {
                private values: map<address, u64>;
                private total: u64;
                private count: u64;
                
                function add_value(v: u64) private {
                    values[msg.sender] = v;
                    total += v;
                    count += 1;
                }
                
                function get_average() private view returns u64 {
                    return total / count;
                }
            }
        "#),
        ("large", r#"
            contract LargeContract {
                private user_data: map<address, UserData>;
                private global_state: GlobalState;
                private permissions: map<address, Permissions>;
                
                struct UserData {
                    balance: u64,
                    last_activity: u64,
                    reputation: u64,
                    metadata: bytes32
                }
                
                struct GlobalState {
                    total_users: u64,
                    total_balance: u64,
                    last_update: u64,
                    system_fee: u64
                }
                
                struct Permissions {
                    can_read: bool,
                    can_write: bool,
                    can_admin: bool,
                    expiry: u64
                }
                
                function register_user(initial_balance: u64) private {
                    require(!user_data.contains(msg.sender));
                    user_data[msg.sender] = UserData {
                        balance: initial_balance,
                        last_activity: block.timestamp,
                        reputation: 100,
                        metadata: hash(msg.sender, block.timestamp)
                    };
                    global_state.total_users += 1;
                    global_state.total_balance += initial_balance;
                }
                
                function transfer(to: address, amount: u64) private {
                    require(user_data[msg.sender].balance >= amount);
                    user_data[msg.sender].balance -= amount;
                    user_data[to].balance += amount;
                    user_data[msg.sender].last_activity = block.timestamp;
                    user_data[to].last_activity = block.timestamp;
                }
            }
        "#),
    ];
    
    for (size, contract_code) in contract_sizes {
        group.bench_with_input(BenchmarkId::new("deploy", size), &contract_code, |b, code| {
            b.to_async(&rt).iter(|| async {
                let mut engine = PrivacyEngine::new().await.unwrap();
                let result = engine.deploy_contract(
                    format!("BenchContract_{}", size),
                    code.to_string(),
                    PrivacyLevel::Medium
                ).await;
                black_box(result)
            })
        });
    }
    
    group.finish();
}

fn bench_zk_proof_generation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("zk_proof_generation");
    
    let proof_systems = vec![
        ProofSystemType::Groth16,
        ProofSystemType::PLONK,
        ProofSystemType::Bulletproofs,
        ProofSystemType::STARK,
    ];
    
    let data_sizes = vec![32, 128, 512, 1024];
    
    for proof_system in proof_systems {
        for &data_size in &data_sizes {
            group.bench_with_input(
                BenchmarkId::new(format!("{:?}", proof_system), data_size),
                &(proof_system, data_size),
                |b, &(ps, size)| {
                    b.to_async(&rt).iter(|| async {
                        let mut engine = PrivacyEngine::new().await.unwrap();
                        let proof_data = vec![1u8; size];
                        let public_inputs = vec![42u64, 100u64];
                        
                        let result = engine.generate_zk_proof(ps, proof_data, public_inputs).await;
                        black_box(result)
                    })
                },
            );
        }
    }
    
    group.finish();
}

fn bench_private_state_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("private_state_operations");
    
    let operations = vec!["create", "read", "update", "delete"];
    let value_sizes = vec![32, 256, 1024, 4096];
    
    for operation in operations {
        for &value_size in &value_sizes {
            group.bench_with_input(
                BenchmarkId::new(operation, value_size),
                &(operation, value_size),
                |b, &(op, size)| {
                    b.to_async(&rt).iter(|| async {
                        let mut engine = PrivacyEngine::new().await.unwrap();
                        let key = format!("bench_key_{}", size);
                        let value = vec![42u8; size];
                        
                        match op {
                            "create" => {
                                let result = engine.create_private_state(
                                    key,
                                    value,
                                    PrivacyLevel::Medium
                                ).await;
                                black_box(result)
                            },
                            "read" => {
                                // First create the state
                                let _ = engine.create_private_state(
                                    key.clone(),
                                    value,
                                    PrivacyLevel::Medium
                                ).await;
                                
                                let result = engine.read_private_state(
                                    key,
                                    PrivacyLevel::Medium
                                ).await;
                                black_box(result)
                            },
                            "update" => {
                                // First create the state
                                let _ = engine.create_private_state(
                                    key.clone(),
                                    vec![1u8; size],
                                    PrivacyLevel::Medium
                                ).await;
                                
                                let result = engine.update_private_state(
                                    key,
                                    value,
                                    PrivacyLevel::Medium
                                ).await;
                                black_box(result)
                            },
                            "delete" => {
                                // First create the state
                                let _ = engine.create_private_state(
                                    key.clone(),
                                    value,
                                    PrivacyLevel::Medium
                                ).await;
                                
                                let result = engine.delete_private_state(
                                    key,
                                    PrivacyLevel::Medium
                                ).await;
                                black_box(result)
                            },
                            _ => unreachable!()
                        }
                    })
                },
            );
        }
    }
    
    group.finish();
}

fn bench_private_transaction_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("private_transaction_processing");
    
    let transaction_counts = vec![1, 10, 50, 100];
    
    for &count in &transaction_counts {
        group.bench_with_input(
            BenchmarkId::new("process_transactions", count),
            &count,
            |b, &tx_count| {
                b.to_async(&rt).iter(|| async {
                    let mut engine = PrivacyEngine::new().await.unwrap();
                    
                    let mut results = Vec::new();
                    for i in 0..tx_count {
                        let transaction = PrivateTransaction::new(
                            format!("sender_{}", i),
                            format!("recipient_{}", i),
                            1000u64 + i as u64,
                            PrivacyLevel::Medium,
                            Some(format!("Transaction {}", i))
                        );
                        
                        let result = engine.process_private_transaction(transaction).await;
                        results.push(result);
                    }
                    
                    black_box(results)
                })
            },
        );
    }
    
    group.finish();
}

fn bench_selective_disclosure(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("selective_disclosure");
    
    let field_counts = vec![5, 10, 20, 50];
    
    for &field_count in &field_counts {
        group.bench_with_input(
            BenchmarkId::new("create_disclosure", field_count),
            &field_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let mut engine = PrivacyEngine::new().await.unwrap();
                    
                    // Create transaction data with many fields
                    let mut transaction_data = HashMap::new();
                    let mut disclosure_rules = Vec::new();
                    
                    for i in 0..count {
                        let field_name = format!("field_{}", i);
                        let field_value = format!("value_{}", i);
                        transaction_data.insert(field_name.clone(), field_value);
                        disclosure_rules.push((field_name, i % 2 == 0)); // Disclose every other field
                    }
                    
                    let disclosure_policy = DisclosurePolicy::new(disclosure_rules);
                    
                    let result = engine.create_selective_disclosure(
                        transaction_data,
                        disclosure_policy
                    ).await;
                    
                    black_box(result)
                })
            },
        );
    }
    
    group.finish();
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_operations");
    
    let concurrency_levels = vec![2, 4, 8, 16];
    
    for &concurrency in &concurrency_levels {
        group.bench_with_input(
            BenchmarkId::new("concurrent_contract_execution", concurrency),
            &concurrency,
            |b, &conc_level| {
                b.to_async(&rt).iter(|| async {
                    let engine = PrivacyEngine::new().await.unwrap();
                    
                    // Deploy a contract first
                    let contract_code = r#"
                        contract BenchContract {
                            private counter: u64;
                            function increment() private { counter += 1; }
                        }
                    "#;
                    
                    let contract_id = engine.clone().deploy_contract(
                        "BenchContract".to_string(),
                        contract_code.to_string(),
                        PrivacyLevel::Medium
                    ).await.unwrap();
                    
                    // Execute concurrent operations
                    let mut handles = Vec::new();
                    
                    for i in 0..conc_level {
                        let engine_clone = engine.clone();
                        let contract_id_clone = contract_id.clone();
                        
                        let handle = tokio::spawn(async move {
                            let mut call_data = HashMap::new();
                            call_data.insert("function".to_string(), "increment".to_string());
                            
                            engine_clone.execute_private_contract(
                                contract_id_clone,
                                call_data,
                                PrivacyLevel::Medium
                            ).await
                        });
                        
                        handles.push(handle);
                    }
                    
                    let results = futures::future::join_all(handles).await;
                    black_box(results)
                })
            },
        );
    }
    
    group.finish();
}

fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("memory_usage_large_state", |b| {
        b.to_async(&rt).iter(|| async {
            let mut engine = PrivacyEngine::new().await.unwrap();
            
            // Create many private state entries
            for i in 0..1000 {
                let key = format!("large_state_key_{}", i);
                let value = vec![i as u8; 1024]; // 1KB per entry
                
                let _ = engine.create_private_state(
                    key,
                    value,
                    PrivacyLevel::Low
                ).await;
            }
            
            // Get metrics to measure memory usage
            let metrics = engine.get_metrics().await;
            black_box(metrics)
        })
    });
}

criterion_group!(
    benches,
    bench_privacy_engine_initialization,
    bench_contract_deployment,
    bench_zk_proof_generation,
    bench_private_state_operations,
    bench_private_transaction_processing,
    bench_selective_disclosure,
    bench_concurrent_operations,
    bench_memory_usage
);

criterion_main!(benches);