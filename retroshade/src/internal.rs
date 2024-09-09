use sha2::{Digest, Sha256};
use soroban_env_host::{
    budget::Budget,
    e2e_invoke::{
        invoke_host_function, ledger_entry_to_ledger_key, LedgerEntryChange,
        LedgerEntryLiveUntilChange,
    },
    xdr::{
        AccountId, ContractDataDurability, ContractDataEntry, ContractEvent, ContractExecutable,
        ContractIdPreimage, ContractIdPreimageFromAddress, CreateContractArgs, DiagnosticEvent,
        ExtensionPoint, HashIdPreimage, HashIdPreimageSorobanAuthorization, HostFunction,
        InvokeContractArgs, LedgerEntry, LedgerEntryData, LedgerFootprint, LedgerKey,
        LedgerKeyContractCode, LedgerKeyContractData, Limits, ReadXdr, ScAddress, ScErrorCode,
        ScErrorType, ScMap, ScVal, ScVec, SorobanAuthorizationEntry, SorobanCredentials,
        SorobanResources, TtlEntry, Uint256, WriteXdr,
    },
    zephyr::RetroshadeExport,
    Host, HostError, LedgerInfo,
};

#[derive(Debug, Eq, PartialEq, Clone)]
struct LedgerEntryChangeHelper {
    read_only: bool,
    key: LedgerKey,
    old_entry_size_bytes: u32,
    new_value: Option<LedgerEntry>,
    ttl_change: Option<LedgerEntryLiveUntilChange>,
}

impl From<LedgerEntryChange> for LedgerEntryChangeHelper {
    fn from(c: LedgerEntryChange) -> Self {
        Self {
            read_only: c.read_only,
            key: LedgerKey::from_xdr(c.encoded_key, Limits::none()).unwrap(),
            old_entry_size_bytes: c.old_entry_size_bytes,
            new_value: c
                .encoded_new_value
                .map(|v| LedgerEntry::from_xdr(v, Limits::none()).unwrap()),
            ttl_change: c.ttl_change,
        }
    }
}

impl LedgerEntryChangeHelper {
    fn no_op_change(entry: &LedgerEntry, live_until_ledger: u32) -> Self {
        let ledger_key = ledger_entry_to_ledger_key(entry, &Budget::default()).unwrap();
        let durability = match &ledger_key {
            LedgerKey::ContractData(cd) => Some(cd.durability),
            LedgerKey::ContractCode(_) => Some(ContractDataDurability::Persistent),
            _ => None,
        };
        Self {
            read_only: true,
            key: ledger_key.clone(),
            old_entry_size_bytes: entry.to_xdr(Limits::none()).unwrap().len() as u32,
            new_value: None,
            ttl_change: if let Some(durability) = durability {
                Some(LedgerEntryLiveUntilChange {
                    key_hash: compute_key_hash(&ledger_key),
                    durability,
                    old_live_until_ledger: live_until_ledger,
                    new_live_until_ledger: live_until_ledger,
                })
            } else {
                None
            },
        }
    }
}

#[derive(Debug)]
pub struct InvokeHostFunctionHelperResult {
    pub invoke_result: Result<ScVal, HostError>,
    pub ledger_changes: Vec<LedgerEntryChangeHelper>,
    pub contract_events: Vec<ContractEvent>,
    pub diagnostic_events: Vec<DiagnosticEvent>,
    pub retroshades: Vec<RetroshadeExport>,
    pub budget: Budget,
}

fn compute_key_hash(key: &LedgerKey) -> Vec<u8> {
    let key_xdr = key.to_xdr(Limits::none()).unwrap();
    let hash: [u8; 32] = Sha256::digest(&key_xdr).into();
    hash.to_vec()
}

fn ttl_entry(key: &LedgerKey, ttl: u32) -> TtlEntry {
    TtlEntry {
        key_hash: compute_key_hash(key).try_into().unwrap(),
        live_until_ledger_seq: ttl,
    }
}

pub fn execute_svm(
    enable_diagnostics: bool,
    host_fn: &HostFunction,
    resources: &SorobanResources,
    source_account: &AccountId,
    auth_entries: Vec<SorobanAuthorizationEntry>,
    ledger_info: &LedgerInfo,
    ledger_entries_with_ttl: Vec<(LedgerEntry, Option<u32>)>,
    prng_seed: &[u8; 32],
) -> Result<InvokeHostFunctionHelperResult, HostError> {
    let limits = Limits::none();
    let encoded_host_fn = host_fn.to_xdr(limits.clone()).unwrap();
    let encoded_resources = resources.to_xdr(limits.clone()).unwrap();
    let encoded_source_account = source_account.to_xdr(limits.clone()).unwrap();
    let encoded_auth_entries: Vec<Vec<u8>> = auth_entries
        .iter()
        .map(|e| e.to_xdr(limits.clone()).unwrap())
        .collect();
    let encoded_ledger_entries: Vec<Vec<u8>> = ledger_entries_with_ttl
        .iter()
        .map(|e| e.0.to_xdr(limits.clone()).unwrap())
        .collect();
    let encoded_ttl_entries: Vec<Vec<u8>> = ledger_entries_with_ttl
        .iter()
        .map(|e| {
            let (le, ttl) = e;
            let key = match &le.data {
                LedgerEntryData::ContractData(cd) => {
                    LedgerKey::ContractData(LedgerKeyContractData {
                        contract: cd.contract.clone(),
                        key: cd.key.clone(),
                        durability: cd.durability,
                    })
                }
                LedgerEntryData::ContractCode(code) => {
                    LedgerKey::ContractCode(LedgerKeyContractCode {
                        hash: code.hash.clone(),
                    })
                }
                _ => {
                    return vec![];
                }
            };
            ttl_entry(&key, ttl.unwrap())
                .to_xdr(limits.clone())
                .unwrap()
        })
        .collect();
    let budget = Budget::default();

    budget.reset_unlimited()?;

    let mut diagnostic_events = Vec::<DiagnosticEvent>::new();
    let res = invoke_host_function(
        &budget,
        enable_diagnostics,
        encoded_host_fn,
        encoded_resources,
        encoded_source_account,
        encoded_auth_entries.into_iter(),
        ledger_info.clone(),
        encoded_ledger_entries.into_iter(),
        encoded_ttl_entries.into_iter(),
        prng_seed.to_vec(),
        &mut diagnostic_events,
    )?;
    Ok(InvokeHostFunctionHelperResult {
        invoke_result: res
            .encoded_invoke_result
            .map(|v| ScVal::from_xdr(v, limits.clone()).unwrap()),
        ledger_changes: res.ledger_changes.into_iter().map(|c| c.into()).collect(),
        contract_events: res
            .encoded_contract_events
            .iter()
            .map(|v| ContractEvent::from_xdr(v, limits.clone()).unwrap())
            .collect(),
        diagnostic_events,
        budget,
        retroshades: res.retroshades,
    })
}
