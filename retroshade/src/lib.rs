use std::{collections::HashMap, rc::Rc};

use conversion::FromScVal;
use internal::{execute_svm, execute_svm_in_recording_mode};
pub use soroban_env_host;
use soroban_env_host::{
    storage::SnapshotSource,
    xdr::{
        AccountId, DiagnosticEvent, Hash, HostFunction, LedgerEntry, ScVal,
        SorobanAuthorizationEntry, SorobanResources, TransactionMetaV3, TransactionV1Envelope,
    },
    zephyr::RetroshadeExport,
    HostError, LedgerInfo,
};
pub mod conversion;
mod internal;
mod state;

#[cfg(test)]
mod test;

pub struct RetroshadesExecution {
    /// Pre-tx-execution state.
    target_pre_execution_state: Vec<(LedgerEntry, Option<u32>)>,

    /// Transaction's host function.
    host_function: Option<HostFunction>,

    /// Tx's authorization entries.
    auth_entries: Vec<SorobanAuthorizationEntry>,

    /// Tx's soroban resources.
    resources: Option<SorobanResources>,

    /// Operation's source account.
    source_account: Option<AccountId>,

    /// Ledger information.
    ledger_info: LedgerInfo,
}

#[derive(Clone, Debug)]
pub enum RetroshadeError {
    SVMHost(HostError),
    NotSorobanTx,
    EntryNotFound,
    MissingContext,
    MalformedXdr,
    MalformedRetroshadeEvent,
}

#[derive(Clone, Debug)]
pub struct RetroshadeExecutionResult {
    pub retroshades: Vec<RetroshadeExport>,
    pub diagnostic: Vec<DiagnosticEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedEventEntry {
    pub name: String,
    pub value: FromScVal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetroshadeExportPretty {
    pub contract_id: String,
    pub target: String,
    pub event: Vec<PackedEventEntry>,
}

#[derive(Clone, Debug)]
pub struct RetroshadeExecutionResultPretty {
    pub retroshades: Vec<RetroshadeExportPretty>,
    pub diagnostic: Vec<DiagnosticEvent>,
}

/// The ideal flow would be:
/// -- Mercury --
/// 1. We get the ledgermeta xdr
/// 2. Given the low frequency of soroban txs in stellar, for each soroban
/// invocation we check wether the database registry contains contracts
/// associated to the invocation footprint. Keep also track of these contracts
/// and their respective owner (user_id). If so:
/// -- Retroshade lib
/// 3. Using an implementor-provided snapshotsource, get all of the entries
/// requested in the footprint.
/// 3.5. For each entry in the operation meta that got updated, replace within
/// the in-memory state entries that were updated and revert them back to the
/// corresponding `state`.
/// 4. Replace all code entries of contracts ids that are also Mercury-deployed
/// with their Mercury counterpart.
/// ~ execution state is now ready ~
/// 5. From the txenvelope, extract the host function, resources, source acccount
/// and auth entries.
/// 6. Build a correct ledger info object.
/// 7. Execute the original host function and gather the retroshades.
/// 8. Process the retroshades: get the event map and build a table object
/// making the columns the map's keys, and the row the map's values.
/// 8.5 Iterate of each of the map's values to extrat the inner values mapping
/// them to known sql types (e.g convert vec<symbol> to a TEXT[], bytes to hex TEXT
/// bool to bool, void to null, numeric (including i128, timepoint, duration, u/i256)
/// to bigint, address to text, maps get converted to json, and vectors convert inner
/// values to string and get converted to TEXT[]).
/// -- Mercury --
/// 9. For each of the retroshades, match the contract id owner, build the
/// table insert or update (name depending on the target) and then push to
/// the database.

impl RetroshadesExecution {
    pub fn new(ledger_info: LedgerInfo) -> Self {
        Self {
            target_pre_execution_state: vec![],
            host_function: None,
            auth_entries: vec![],
            resources: None,
            source_account: None,
            ledger_info,
        }
    }

    pub fn build_from_envelope_and_meta(
        &mut self,
        snapshot_source: Box<dyn SnapshotSource>,
        tx_envelope: TransactionV1Envelope,
        tx_meta: TransactionMetaV3,
        mercury_contracts: HashMap<Hash, &[u8]>,
    ) -> Result<bool, RetroshadeError> {
        self.build_current_state(snapshot_source, tx_envelope)?;
        self.state_reset_to_pre_execution(tx_meta)?;

        self.replace_binaries(mercury_contracts)
    }

    pub fn retroshade(&self) -> Result<RetroshadeExecutionResult, RetroshadeError> {
        let svm_execution = execute_svm(
            true,
            self.host_function
                .as_ref()
                .ok_or(RetroshadeError::MissingContext)?,
            self.resources
                .as_ref()
                .ok_or(RetroshadeError::MissingContext)?,
            self.source_account
                .as_ref()
                .ok_or(RetroshadeError::MissingContext)?,
            self.auth_entries.clone(),
            &self.ledger_info,
            self.target_pre_execution_state.clone(),
            &rand::random::<[u8; 32]>(),
        );

        match svm_execution {
            Ok(result) => Ok(RetroshadeExecutionResult {
                retroshades: result.retroshades,
                diagnostic: result.diagnostic_events,
            }),
            Err(host_error) => Err(RetroshadeError::SVMHost(host_error)),
        }
    }

    pub fn retroshade_recording(
        &self,
        ledger_snapshot: Rc<dyn SnapshotSource>,
    ) -> Result<RetroshadeExecutionResult, RetroshadeError> {
        let svm_execution = execute_svm_in_recording_mode(
            true,
            self.host_function
                .as_ref()
                .ok_or(RetroshadeError::MissingContext)?,
            self.source_account
                .as_ref()
                .ok_or(RetroshadeError::MissingContext)?,
            self.ledger_info.clone(),
            rand::random::<[u8; 32]>(),
            ledger_snapshot,
        );

        match svm_execution {
            Ok(result) => Ok(RetroshadeExecutionResult {
                retroshades: result.retroshades,
                diagnostic: result.diagnostic_events,
            }),
            Err(host_error) => Err(RetroshadeError::SVMHost(host_error)),
        }
    }

    /// Perfect for exporting to SQL databases.
    pub fn retroshade_packed(&self) -> Result<RetroshadeExecutionResultPretty, RetroshadeError> {
        let retroshade_exec = self.retroshade()?;

        println!("Checking successful contract call");
        if let Some(first) = retroshade_exec.diagnostic.get(0) {
            if !first.in_successful_contract_call {
                println!(
                    "\nError while executing retroshades: \n{:?}",
                    retroshade_exec.diagnostic
                )
            }
        }

        let mut pretty_retroshades = Vec::new();

        for retroshade in retroshade_exec.retroshades {
            let mut packed_event_entries = Vec::new();

            let map_entry = if let ScVal::Map(Some(map)) = retroshade.event_object {
                map
            } else {
                return Err(RetroshadeError::MalformedRetroshadeEvent);
            };

            for key_value in map_entry.0.to_vec() {
                let packed_entry = PackedEventEntry {
                    name: if let ScVal::Symbol(symbol) = key_value.key {
                        symbol.to_string()
                    } else {
                        return Err(RetroshadeError::MalformedRetroshadeEvent);
                    },
                    value: key_value.val.into(),
                };

                packed_event_entries.push(packed_entry);
            }

            let pretty = RetroshadeExportPretty {
                contract_id: stellar_strkey::Contract(retroshade.contract_id.0).to_string(),
                target: if let ScVal::Symbol(symbol) = retroshade.target {
                    symbol.to_string()
                } else {
                    return Err(RetroshadeError::MalformedRetroshadeEvent);
                },
                event: packed_event_entries,
            };

            pretty_retroshades.push(pretty)
        }

        Ok(RetroshadeExecutionResultPretty {
            retroshades: pretty_retroshades,
            diagnostic: retroshade_exec.diagnostic,
        })
    }
}
