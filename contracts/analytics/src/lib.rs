#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Map, String, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotMetadata {
    pub epoch: u64,
    pub timestamp: u64,
    pub hash: BytesN<32>,
    // Extendable for future fields
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotSubmittedEvent {
    pub epoch: u64,
    pub hash: BytesN<32>,
    pub submitter: Address,
    pub timestamp: u64,
    pub previous_epoch: u64,
    pub ledger_sequence: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseEvent {
    pub paused_by: Address,
    pub reason: String,
    pub timestamp: u64,
    pub ledger_sequence: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnpauseEvent {
    pub unpaused_by: Address,
    pub reason: String,
    pub timestamp: u64,
    pub ledger_sequence: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelockAction {
    pub action_type: String,
    pub new_admin: Address,
    pub proposer: Address,
    pub proposed_at: u64,
    pub executable_at: u64,
    pub executed: bool,
}

const TIMELOCK_DELAY: u64 = 172800; // 48 hours in seconds

#[contracttype]
pub enum DataKey {
    /// Authorized submitter address (only this address can submit snapshots)
    Admin,
    /// Map of epoch -> snapshot metadata (persistent storage for full history)
    Snapshots,
    /// Latest epoch number (instance storage for quick access)
    LatestEpoch,
    /// Emergency pause state (true = paused, false = active)
    Paused,
    /// Governance contract address (only it can call set_admin_by_governance / set_paused_by_governance)
    Governance,
    /// Auto-incrementing ID counter for timelock actions
    NextActionId,
    /// Timelock action keyed by action ID
    TimelockAction(u64),
}

#[contract]
pub struct AnalyticsContract;

#[contractimpl]
impl AnalyticsContract {
    /// Initialize contract storage with an authorized admin address
    /// Sets up empty snapshot history and initializes latest epoch to 0
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `admin` - Address authorized to submit snapshots
    ///
    /// # Panics
    /// * If contract is already initialized (admin already set)
    pub fn initialize(env: Env, admin: Address) {
        let storage = env.storage().instance();

        // Prevent re-initialization if admin is already set
        if storage.has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }

        // Store the authorized admin address
        storage.set(&DataKey::Admin, &admin);

        // Initialize latest epoch to 0
        storage.set(&DataKey::LatestEpoch, &0u64);

        // Initialize contract as not paused
        storage.set(&DataKey::Paused, &false);

        // Initialize empty snapshots map
        let persistent_storage = env.storage().persistent();
        let empty_snapshots = Map::<u64, SnapshotMetadata>::new(&env);
        persistent_storage.set(&DataKey::Snapshots, &empty_snapshots);
    }

    /// Submit a new snapshot for a specific epoch.
    /// Stores the snapshot in the historical map and updates latest epoch.
    /// Epochs must be submitted in strictly increasing order (monotonicity).
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `epoch` - Epoch identifier (must be positive and strictly greater than latest)
    /// * `hash` - 32-byte hash of the analytics snapshot
    /// * `caller` - Address attempting to submit (must be the authorized admin)
    ///
    /// # Panics
    /// * If contract is paused for emergency maintenance
    /// * If admin is not set (contract not initialized)
    /// * If caller is not the authorized admin
    /// * If epoch is 0 (invalid)
    /// * If epoch <= latest (monotonicity violated: out-of-order or duplicate)
    ///
    /// # Returns
    /// * Ledger timestamp when snapshot was recorded
    pub fn submit_snapshot(env: Env, epoch: u64, hash: BytesN<32>, caller: Address) -> u64 {
        // Check if contract is paused
        let is_paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if is_paused {
            panic!("Contract is paused for emergency maintenance");
        }

        // Require authentication from the caller
        caller.require_auth();

        // Verify caller is the authorized admin
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin not set");

        if caller != admin {
            panic!("Unauthorized: only the admin can submit snapshots");
        }

        if epoch == 0 {
            panic!("Invalid epoch: must be greater than 0");
        }

        let latest: u64 = env
            .storage()
            .instance()
            .get(&DataKey::LatestEpoch)
            .unwrap_or(0);

        if epoch <= latest {
            if epoch == latest {
                panic!("Snapshot for epoch {} already exists", epoch);
            } else {
                panic!(
                    "Epoch monotonicity violated: epoch {} must be strictly greater than latest {}",
                    epoch, latest
                );
            }
        }

        let timestamp = env.ledger().timestamp();
        let ledger_sequence = env.ledger().sequence();
        let metadata = SnapshotMetadata {
            epoch,
            timestamp,
            hash: hash.clone(),
        };

        let mut snapshots: Map<u64, SnapshotMetadata> = env
            .storage()
            .persistent()
            .get(&DataKey::Snapshots)
            .unwrap_or_else(|| Map::new(&env));

        // Defense-in-depth: explicitly prevent overwriting an existing snapshot
        if snapshots.contains_key(epoch) {
            panic!("Snapshot immutability violated: epoch {} already exists in storage", epoch);
        }

        snapshots.set(epoch, metadata);
        env.storage()
            .persistent()
            .set(&DataKey::Snapshots, &snapshots);
        env.storage().instance().set(&DataKey::LatestEpoch, &epoch);

        env.events().publish(
            (symbol_short!("snapshot"), caller.clone()),
            SnapshotSubmittedEvent {
                epoch,
                hash,
                submitter: caller,
                timestamp,
                previous_epoch: latest,
                ledger_sequence,
            },
        );

        timestamp
    }

    /// Get snapshot metadata for a specific epoch
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `epoch` - Epoch to retrieve
    ///
    /// # Returns
    /// * Snapshot metadata for the epoch, or None if not found
    pub fn get_snapshot(env: Env, epoch: u64) -> Option<SnapshotMetadata> {
        let snapshots: Map<u64, SnapshotMetadata> = env
            .storage()
            .persistent()
            .get(&DataKey::Snapshots)
            .unwrap_or_else(|| Map::new(&env));

        snapshots.get(epoch)
    }

    /// Get the latest snapshot metadata
    ///
    /// # Arguments
    /// * `env` - Contract environment
    ///
    /// # Returns
    /// * Latest snapshot metadata, or None if no snapshots exist
    pub fn get_latest_snapshot(env: Env) -> Option<SnapshotMetadata> {
        let latest_epoch: u64 = env
            .storage()
            .instance()
            .get(&DataKey::LatestEpoch)
            .unwrap_or(0);

        if latest_epoch == 0 {
            return None;
        }

        Self::get_snapshot(env, latest_epoch)
    }

    /// Get the complete snapshot history as a Map
    ///
    /// # Arguments
    /// * `env` - Contract environment
    ///
    /// # Returns
    /// * Map of all snapshots keyed by epoch
    pub fn get_snapshot_history(env: Env) -> Map<u64, SnapshotMetadata> {
        env.storage()
            .persistent()
            .get(&DataKey::Snapshots)
            .unwrap_or_else(|| Map::new(&env))
    }

    /// Get the latest epoch number
    ///
    /// # Arguments
    /// * `env` - Contract environment
    ///
    /// # Returns
    /// * Latest epoch number (0 if no snapshots)
    pub fn get_latest_epoch(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::LatestEpoch)
            .unwrap_or(0)
    }

    /// Get all epochs that have snapshots (for iteration purposes)
    ///
    /// # Arguments
    /// * `env` - Contract environment
    ///
    /// # Returns
    /// * Vector of all epochs with stored snapshots
    pub fn get_all_epochs(env: Env) -> soroban_sdk::Vec<u64> {
        let snapshots = Self::get_snapshot_history(env.clone());
        let mut epochs = soroban_sdk::Vec::new(&env);

        for (epoch, _) in snapshots.iter() {
            epochs.push_back(epoch);
        }

        epochs
    }

    /// Get the current authorized admin address
    ///
    /// # Arguments
    /// * `env` - Contract environment
    ///
    /// # Returns
    /// * The admin address if set, None otherwise
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    /// Update the authorized admin address
    /// Only the current admin can transfer admin rights to a new address
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `current_admin` - Current admin address (must authenticate)
    /// * `new_admin` - New address to set as admin
    ///
    /// # Panics
    /// * If contract is not initialized (admin not set)
    /// * If caller is not the current admin
    pub fn set_admin(env: Env, current_admin: Address, new_admin: Address) {
        // Require authentication from the current admin
        current_admin.require_auth();

        // Verify caller is the current admin
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin not set");

        if current_admin != admin {
            panic!("Unauthorized: only the current admin can set a new admin");
        }

        // Update admin address
        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }

    /// Emergency pause the contract
    ///
    /// Pauses all snapshot submissions. Only the admin can pause the contract.
    /// Read operations remain available during pause.
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `caller` - Address attempting to pause (must be admin)
    ///
    /// # Panics
    /// * If contract is not initialized (admin not set)
    /// * If caller is not the admin
    pub fn pause(env: Env, caller: Address, reason: String) {
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin not set");

        if caller != admin {
            panic!("Unauthorized: only the admin can pause the contract");
        }

        env.storage().instance().set(&DataKey::Paused, &true);

        env.events().publish(
            (symbol_short!("pause"), caller.clone()),
            PauseEvent {
                paused_by: caller,
                reason,
                timestamp: env.ledger().timestamp(),
                ledger_sequence: env.ledger().sequence(),
            },
        );
    }

    /// Unpause the contract
    ///
    /// Resumes normal operations. Only the admin can unpause the contract.
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `caller` - Address attempting to unpause (must be admin)
    ///
    /// # Panics
    /// * If contract is not initialized (admin not set)
    /// * If caller is not the admin
    pub fn unpause(env: Env, caller: Address, reason: String) {
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin not set");

        if caller != admin {
            panic!("Unauthorized: only the admin can unpause the contract");
        }

        env.storage().instance().set(&DataKey::Paused, &false);

        env.events().publish(
            (symbol_short!("unpause"), caller.clone()),
            UnpauseEvent {
                unpaused_by: caller,
                reason,
                timestamp: env.ledger().timestamp(),
                ledger_sequence: env.ledger().sequence(),
            },
        );
    }

    /// Set the governance contract address. Only the admin can set this.
    /// The governance contract can then update admin or pause state via voting.
    pub fn set_governance(env: Env, caller: Address, governance: Address) {
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin not set");

        if caller != admin {
            panic!("Unauthorized: only the admin can set governance");
        }

        env.storage()
            .instance()
            .set(&DataKey::Governance, &governance);
    }

    /// Get the current governance contract address (if any).
    pub fn get_governance(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Governance)
    }

    /// Set the admin address. Only the governance contract may call this (after a passed proposal).
    pub fn set_admin_by_governance(env: Env, caller: Address, new_admin: Address) {
        let governance: Address = env
            .storage()
            .instance()
            .get(&DataKey::Governance)
            .expect("Governance not set");

        if caller != governance {
            panic!("Unauthorized: only the governance contract can set admin");
        }

        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }

    /// Set the paused state. Only the governance contract may call this (after a passed proposal).
    pub fn set_paused_by_governance(env: Env, caller: Address, paused: bool) {
        let governance: Address = env
            .storage()
            .instance()
            .get(&DataKey::Governance)
            .expect("Governance not set");

        if caller != governance {
            panic!("Unauthorized: only the governance contract can set paused");
        }

        env.storage().instance().set(&DataKey::Paused, &paused);
    }

    /// Batch submit multiple snapshots in a single transaction.
    /// Epochs must be strictly increasing within the batch and relative to the current latest.
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `caller` - Address attempting to submit (must be the authorized admin)
    /// * `snapshots` - Vector of (epoch, hash) pairs to submit
    ///
    /// # Returns
    /// * Vector of ledger timestamps for each submitted snapshot
    pub fn batch_submit_snapshots(
        env: Env,
        caller: Address,
        snapshots: Vec<(u64, BytesN<32>)>,
    ) -> Vec<u64> {
        let is_paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if is_paused {
            panic!("Contract is paused for emergency maintenance");
        }

        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin not set");

        if caller != admin {
            panic!("Unauthorized: only the admin can submit snapshots");
        }

        let mut timestamps = Vec::new(&env);
        let mut snapshots_map: Map<u64, SnapshotMetadata> = env
            .storage()
            .persistent()
            .get(&DataKey::Snapshots)
            .unwrap_or_else(|| Map::new(&env));
        let mut latest: u64 = env
            .storage()
            .instance()
            .get(&DataKey::LatestEpoch)
            .unwrap_or(0);

        for (epoch, hash) in snapshots.iter() {
            if epoch == 0 {
                panic!("Invalid epoch: must be greater than 0");
            }
            if epoch <= latest {
                panic!("Epoch monotonicity violated: epoch must be strictly greater than latest");
            }
            if snapshots_map.contains_key(epoch) {
                panic!("Snapshot immutability violated: epoch already exists in storage");
            }

            let timestamp = env.ledger().timestamp();
            snapshots_map.set(
                epoch,
                SnapshotMetadata {
                    epoch,
                    timestamp,
                    hash,
                },
            );
            latest = epoch;
            timestamps.push_back(timestamp);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Snapshots, &snapshots_map);
        env.storage().instance().set(&DataKey::LatestEpoch, &latest);

        env.events()
            .publish((symbol_short!("batch"), caller), snapshots.len());

        timestamps
    }

    /// Batch get multiple snapshots by epoch in a single call.
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `epochs` - Vector of epoch numbers to retrieve
    ///
    /// # Returns
    /// * Vector of Option<SnapshotMetadata> (None for epochs not found)
    pub fn batch_get_snapshots(
        env: Env,
        epochs: Vec<u64>,
    ) -> Vec<Option<SnapshotMetadata>> {
        let snapshots: Map<u64, SnapshotMetadata> = env
            .storage()
            .persistent()
            .get(&DataKey::Snapshots)
            .unwrap_or_else(|| Map::new(&env));

        let mut results = Vec::new(&env);
        for epoch in epochs.iter() {
            results.push_back(snapshots.get(epoch));
        }
        results
    }

    /// Propose an admin change with a 48-hour timelock.
    /// Only the current admin can propose. Returns the action ID.
    pub fn propose_admin_change(env: Env, proposer: Address, new_admin: Address) -> u64 {
        proposer.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin not set");

        if proposer != admin {
            panic!("Unauthorized: only the admin can propose changes");
        }

        let action_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextActionId)
            .unwrap_or(0u64);

        let now = env.ledger().timestamp();
        let action = TimelockAction {
            action_type: String::from_str(&env, "set_admin"),
            new_admin: new_admin.clone(),
            proposer: proposer.clone(),
            proposed_at: now,
            executable_at: now + TIMELOCK_DELAY,
            executed: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::TimelockAction(action_id), &action);
        env.storage()
            .instance()
            .set(&DataKey::NextActionId, &(action_id + 1));

        env.events().publish(
            (symbol_short!("propose"), proposer),
            (action_id, new_admin, action.executable_at),
        );

        action_id
    }

    /// Execute a timelock action after the delay has passed.
    pub fn execute_timelock_action(env: Env, executor: Address, action_id: u64) {
        executor.require_auth();

        let mut action: TimelockAction = env
            .storage()
            .persistent()
            .get(&DataKey::TimelockAction(action_id))
            .expect("Action not found");

        if env.ledger().timestamp() < action.executable_at {
            panic!("Timelock not expired: action cannot be executed yet");
        }

        if action.executed {
            panic!("Action already executed");
        }

        env.storage()
            .instance()
            .set(&DataKey::Admin, &action.new_admin);

        action.executed = true;
        env.storage()
            .persistent()
            .set(&DataKey::TimelockAction(action_id), &action);

        env.events()
            .publish((symbol_short!("execute"), executor), action_id);
    }

    /// Cancel a pending timelock action. Only the current admin can cancel.
    pub fn cancel_timelock_action(env: Env, admin: Address, action_id: u64) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin not set");

        if admin != stored_admin {
            panic!("Unauthorized: only the admin can cancel actions");
        }

        env.storage()
            .persistent()
            .remove(&DataKey::TimelockAction(action_id));

        env.events()
            .publish((symbol_short!("cancel"), admin), action_id);
    }

    /// Get a timelock action by ID.
    pub fn get_timelock_action(env: Env, action_id: u64) -> Option<TimelockAction> {
        env.storage()
            .persistent()
            .get(&DataKey::TimelockAction(action_id))
    }

    /// Check if contract is paused
    ///
    /// # Arguments
    /// * `env` - Contract environment
    ///
    /// # Returns
    /// * `true` if contract is paused, `false` otherwise
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests;
