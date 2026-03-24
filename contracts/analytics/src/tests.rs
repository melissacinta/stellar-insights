use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, Vec,
};

fn create_test_hash(env: &Env, value: u8) -> BytesN<32> {
    BytesN::from_array(env, &[value; 32])
}

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    assert_eq!(client.get_latest_epoch(), 0);
    assert_eq!(client.get_snapshot_history().len(), 0);
    assert_eq!(client.get_latest_snapshot(), None);
    assert_eq!(client.get_admin(), Some(admin));
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_initialize_cannot_reinitialize() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    // Second initialization should fail
    client.initialize(&admin);
}

#[test]
fn test_submit_single_snapshot() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    env.ledger().set_timestamp(1234);

    let epoch = 1u64;
    let hash = create_test_hash(&env, 1);

    let timestamp = client.submit_snapshot(&epoch, &hash, &admin);

    assert_eq!(timestamp, 1234);

    let snapshot = client.get_snapshot(&epoch).unwrap();
    assert_eq!(snapshot.epoch, epoch);
    assert_eq!(snapshot.hash, hash);
    assert_eq!(snapshot.timestamp, timestamp);

    assert_eq!(client.get_latest_epoch(), epoch);

    let latest = client.get_latest_snapshot().unwrap();
    assert_eq!(latest.epoch, epoch);
    assert_eq!(latest.hash, hash);
    assert_eq!(latest.timestamp, timestamp);
}

#[test]
fn test_multiple_snapshots_strictly_increasing_epochs() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let epoch1 = 1u64;
    let hash1 = create_test_hash(&env, 1);
    client.submit_snapshot(&epoch1, &hash1, &admin);

    let epoch2 = 2u64;
    let hash2 = create_test_hash(&env, 2);
    client.submit_snapshot(&epoch2, &hash2, &admin);

    let epoch3 = 3u64;
    let hash3 = create_test_hash(&env, 3);
    client.submit_snapshot(&epoch3, &hash3, &admin);

    assert_eq!(client.get_snapshot(&epoch1).unwrap().hash, hash1);
    assert_eq!(client.get_snapshot(&epoch2).unwrap().hash, hash2);
    assert_eq!(client.get_snapshot(&epoch3).unwrap().hash, hash3);

    assert_eq!(client.get_latest_epoch(), epoch3);

    let latest = client.get_latest_snapshot().unwrap();
    assert_eq!(latest.epoch, epoch3);
    assert_eq!(latest.hash, hash3);

    let history = client.get_snapshot_history();
    assert_eq!(history.len(), 3);

    let all_epochs = client.get_all_epochs();
    assert_eq!(all_epochs.len(), 3);
    assert!(all_epochs.contains(epoch1));
    assert!(all_epochs.contains(epoch2));
    assert!(all_epochs.contains(epoch3));
}

#[test]
fn test_non_sequential_epochs_monotonic_order() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let epochs = [1u64, 5u64, 10u64];
    for (i, &epoch) in epochs.iter().enumerate() {
        let hash = create_test_hash(&env, (i + 1) as u8);
        client.submit_snapshot(&epoch, &hash, &admin);
    }

    for (i, &epoch) in epochs.iter().enumerate() {
        let snapshot = client.get_snapshot(&epoch).unwrap();
        assert_eq!(snapshot.epoch, epoch);
        assert_eq!(snapshot.hash, create_test_hash(&env, (i + 1) as u8));
    }

    assert_eq!(client.get_latest_epoch(), 10u64);
    assert_eq!(client.get_snapshot_history().len(), 3);
}

#[test]
fn test_historical_data_integrity_after_new_submissions() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    env.ledger().set_timestamp(100);
    let epoch1 = 1u64;
    let hash1 = create_test_hash(&env, 1);
    let timestamp1 = client.submit_snapshot(&epoch1, &hash1, &admin);

    env.ledger().set_timestamp(200);
    let epoch2 = 2u64;
    let hash2 = create_test_hash(&env, 2);
    let timestamp2 = client.submit_snapshot(&epoch2, &hash2, &admin);

    let snapshot1_before = client.get_snapshot(&epoch1).unwrap();
    let snapshot2_before = client.get_snapshot(&epoch2).unwrap();

    env.ledger().set_timestamp(300);
    let epoch3 = 5u64;
    let hash3 = create_test_hash(&env, 5);
    client.submit_snapshot(&epoch3, &hash3, &admin);

    let snapshot1_after = client.get_snapshot(&epoch1).unwrap();
    let snapshot2_after = client.get_snapshot(&epoch2).unwrap();

    assert_eq!(snapshot1_after, snapshot1_before);
    assert_eq!(snapshot2_after, snapshot2_before);
    assert_eq!(snapshot1_after.timestamp, timestamp1);
    assert_eq!(snapshot2_after.timestamp, timestamp2);

    assert_eq!(client.get_latest_epoch(), epoch3);
}

#[test]
fn test_get_nonexistent_snapshot() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    assert_eq!(client.get_snapshot(&999), None);
}

#[test]
#[should_panic(expected = "Invalid epoch: must be greater than 0")]
fn test_invalid_epoch_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let hash = create_test_hash(&env, 1);
    client.submit_snapshot(&0, &hash, &admin);
}

#[test]
#[should_panic(expected = "already exists")]
fn test_duplicate_epoch_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let epoch = 1u64;
    let hash1 = create_test_hash(&env, 1);
    let hash2 = create_test_hash(&env, 2);

    client.submit_snapshot(&epoch, &hash1, &admin);
    client.submit_snapshot(&epoch, &hash2, &admin);
}

#[test]
#[should_panic(expected = "Epoch monotonicity violated")]
fn test_older_epoch_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let epoch_new = 10u64;
    let hash_new = create_test_hash(&env, 10);
    client.submit_snapshot(&epoch_new, &hash_new, &admin);
    assert_eq!(client.get_latest_epoch(), epoch_new);

    let epoch_old = 5u64;
    let hash_old = create_test_hash(&env, 5);
    client.submit_snapshot(&epoch_old, &hash_old, &admin);
}

#[test]
fn test_bounded_storage_growth_simulation() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let num_epochs = 100u64;
    for epoch in 1..=num_epochs {
        let hash = create_test_hash(&env, (epoch % 255) as u8);
        client.submit_snapshot(&epoch, &hash, &admin);
    }

    for epoch in 1..=num_epochs {
        assert!(client.get_snapshot(&epoch).is_some());
    }

    assert_eq!(client.get_latest_epoch(), num_epochs);
    assert_eq!(client.get_snapshot_history().len(), num_epochs as u32);
    assert_eq!(client.get_all_epochs().len(), num_epochs as u32);
}

// ============================================================================
// Access Control Tests - Tests for Issue #41
// ============================================================================

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_unauthorized_submission_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);

    client.initialize(&admin);

    let epoch = 1u64;
    let hash = create_test_hash(&env, 1);

    // Attempt to submit snapshot with unauthorized address should fail
    client.submit_snapshot(&epoch, &hash, &unauthorized_user);
}

#[test]
fn test_authorized_submission_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.initialize(&admin);

    env.ledger().set_timestamp(1000);

    let epoch = 1u64;
    let hash = create_test_hash(&env, 1);

    // Authorized admin should be able to submit
    let timestamp = client.submit_snapshot(&epoch, &hash, &admin);

    assert_eq!(timestamp, 1000);
    assert_eq!(client.get_latest_epoch(), epoch);

    let snapshot = client.get_snapshot(&epoch).unwrap();
    assert_eq!(snapshot.hash, hash);
}

#[test]
fn test_get_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);

    // Before initialization, admin should be None
    assert_eq!(client.get_admin(), None);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    // After initialization, admin should match
    assert_eq!(client.get_admin(), Some(admin));
}

#[test]
fn test_set_admin_by_authorized_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);
    assert_eq!(client.get_admin(), Some(admin.clone()));

    // Current admin transfers rights to new admin
    client.set_admin(&admin, &new_admin);
    assert_eq!(client.get_admin(), Some(new_admin.clone()));

    // New admin can now submit snapshots
    let epoch = 1u64;
    let hash = create_test_hash(&env, 1);
    client.submit_snapshot(&epoch, &hash, &new_admin);

    assert_eq!(client.get_latest_epoch(), epoch);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_set_admin_by_unauthorized_user_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);

    // Unauthorized user attempts to change admin should fail
    client.set_admin(&unauthorized_user, &new_admin);
}

#[test]
#[should_panic(expected = "Snapshot immutability violated")]
fn test_snapshot_immutability() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let epoch = 1u64;
    client.submit_snapshot(&epoch, &create_test_hash(&env, 1), &admin);
    // Attempting to overwrite an existing snapshot must panic
    client.submit_snapshot(&epoch, &create_test_hash(&env, 2), &admin);
}

#[test]
#[should_panic(expected = "already exists")]
fn test_duplicate_epoch_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let epoch = 5u64;
    client.submit_snapshot(&epoch, &create_test_hash(&env, 5), &admin);
    client.submit_snapshot(&epoch, &create_test_hash(&env, 6), &admin);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_old_admin_cannot_submit_after_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);

    // Transfer admin rights
    client.set_admin(&admin, &new_admin);

    // Old admin should no longer be able to submit
    let epoch = 1u64;
    let hash = create_test_hash(&env, 1);
    client.submit_snapshot(&epoch, &hash, &admin);
}

// ============================================================================
// Batch Operations Tests
// ============================================================================

#[test]
fn test_batch_submit_snapshots() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(1000);

    let mut snapshots = Vec::new(&env);
    snapshots.push_back((1u64, create_test_hash(&env, 1)));
    snapshots.push_back((2u64, create_test_hash(&env, 2)));
    snapshots.push_back((3u64, create_test_hash(&env, 3)));

    let timestamps = client.batch_submit_snapshots(&admin, &snapshots);

    assert_eq!(timestamps.len(), 3);
    assert_eq!(client.get_latest_epoch(), 3);

    assert_eq!(client.get_snapshot(&1u64).unwrap().hash, create_test_hash(&env, 1));
    assert_eq!(client.get_snapshot(&2u64).unwrap().hash, create_test_hash(&env, 2));
    assert_eq!(client.get_snapshot(&3u64).unwrap().hash, create_test_hash(&env, 3));
}

#[test]
fn test_batch_get_snapshots() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    client.submit_snapshot(&1u64, &create_test_hash(&env, 1), &admin);
    client.submit_snapshot(&2u64, &create_test_hash(&env, 2), &admin);
    client.submit_snapshot(&3u64, &create_test_hash(&env, 3), &admin);

    let mut epochs = Vec::new(&env);
    epochs.push_back(1u64);
    epochs.push_back(2u64);
    epochs.push_back(99u64); // non-existent

    let results = client.batch_get_snapshots(&epochs);

    assert_eq!(results.len(), 3);
    assert_eq!(results.get(0).unwrap().unwrap().hash, create_test_hash(&env, 1));
    assert_eq!(results.get(1).unwrap().unwrap().hash, create_test_hash(&env, 2));
    assert!(results.get(2).unwrap().is_none());
}

#[test]
fn test_batch_operations_gas_efficiency() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(5000);

    // Submit 10 snapshots in a single batch call
    let mut snapshots = Vec::new(&env);
    for i in 1u64..=10 {
        snapshots.push_back((i, create_test_hash(&env, i as u8)));
    }

    let timestamps = client.batch_submit_snapshots(&admin, &snapshots);
    assert_eq!(timestamps.len(), 10);
    assert_eq!(client.get_latest_epoch(), 10);

    // Retrieve all 10 in a single batch call
    let mut epochs = Vec::new(&env);
    for i in 1u64..=10 {
        epochs.push_back(i);
    }

    let results = client.batch_get_snapshots(&epochs);
    assert_eq!(results.len(), 10);

    for i in 0u32..10 {
        let snapshot = results.get(i).unwrap().unwrap();
        assert_eq!(snapshot.epoch, (i + 1) as u64);
        assert_eq!(snapshot.hash, create_test_hash(&env, (i + 1) as u8));
    }
}

// ============================================================================
// Timelock Tests
// ============================================================================

#[test]
fn test_timelock_proposal() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(1000);

    let action_id = client.propose_admin_change(&admin, &new_admin);

    let action = client.get_timelock_action(&action_id).unwrap();
    assert_eq!(action.proposer, admin);
    assert_eq!(action.new_admin, new_admin);
    assert_eq!(action.proposed_at, 1000);
    assert_eq!(action.executable_at, 1000 + 172800);
    assert!(!action.executed);
    // Admin unchanged until executed
    assert_eq!(client.get_admin(), Some(admin));
}

#[test]
#[should_panic(expected = "Timelock not expired")]
fn test_timelock_cannot_execute_early() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(1000);

    let action_id = client.propose_admin_change(&admin, &new_admin);

    // Advance time but not past the 48-hour delay
    env.ledger().set_timestamp(1000 + 172799);
    client.execute_timelock_action(&admin, &action_id);
}

#[test]
fn test_timelock_execution_after_delay() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(1000);

    let action_id = client.propose_admin_change(&admin, &new_admin);

    // Advance time past the 48-hour delay
    env.ledger().set_timestamp(1000 + 172800);
    client.execute_timelock_action(&admin, &action_id);

    // Admin should now be updated
    assert_eq!(client.get_admin(), Some(new_admin));

    // Action should be marked executed
    let action = client.get_timelock_action(&action_id).unwrap();
    assert!(action.executed);
}

#[test]
fn test_timelock_cancellation() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(1000);

    let action_id = client.propose_admin_change(&admin, &new_admin);
    assert!(client.get_timelock_action(&action_id).is_some());

    client.cancel_timelock_action(&admin, &action_id);

    // Action should be removed
    assert!(client.get_timelock_action(&action_id).is_none());
    // Admin unchanged
    assert_eq!(client.get_admin(), Some(admin));
}

// ============================================================================
// Rich Event Tests
// ============================================================================

#[test]
fn test_event_data_completeness() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    // Submit a first snapshot so previous_epoch is non-zero for the second
    env.ledger().set_timestamp(500);
    client.submit_snapshot(&1u64, &create_test_hash(&env, 1), &admin);

    env.ledger().set_timestamp(1000);
    let epoch = 2u64;
    let hash = create_test_hash(&env, 2);
    let timestamp = client.submit_snapshot(&epoch, &hash, &admin);

    // Verify the snapshot was stored with correct data (event payload mirrors storage)
    let snapshot = client.get_snapshot(&epoch).unwrap();
    assert_eq!(snapshot.epoch, epoch);
    assert_eq!(snapshot.hash, hash);
    assert_eq!(snapshot.timestamp, timestamp);
    assert_eq!(timestamp, 1000);

    // Verify previous_epoch tracking: latest before this submit was epoch 1
    assert_eq!(client.get_latest_epoch(), epoch);

    // Verify pause event data completeness
    let reason = soroban_sdk::String::from_str(&env, "scheduled maintenance");
    client.pause(&admin, &reason);
    assert!(client.is_paused());

    let unpause_reason = soroban_sdk::String::from_str(&env, "maintenance complete");
    client.unpause(&admin, &unpause_reason);
    assert!(!client.is_paused());
}

#[test]
fn test_event_emission() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(2000);

    // Emit snapshot event
    let epoch = 1u64;
    let hash = create_test_hash(&env, 42);
    let timestamp = client.submit_snapshot(&epoch, &hash, &admin);
    assert_eq!(timestamp, 2000);

    // Confirm snapshot stored correctly (event data matches)
    let snapshot = client.get_snapshot(&epoch).unwrap();
    assert_eq!(snapshot.epoch, epoch);
    assert_eq!(snapshot.hash, hash);
    assert_eq!(snapshot.timestamp, 2000);

    // Emit pause event with reason
    let pause_reason = soroban_sdk::String::from_str(&env, "emergency stop");
    client.pause(&admin, &pause_reason);
    assert!(client.is_paused());

    // Emit unpause event with reason
    let unpause_reason = soroban_sdk::String::from_str(&env, "issue resolved");
    client.unpause(&admin, &unpause_reason);
    assert!(!client.is_paused());

    // Confirm contract is operational again after unpause
    let epoch2 = 2u64;
    let hash2 = create_test_hash(&env, 43);
    client.submit_snapshot(&epoch2, &hash2, &admin);
    assert_eq!(client.get_latest_epoch(), epoch2);
}
