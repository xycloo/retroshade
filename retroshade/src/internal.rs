use std::rc::Rc;

use sha2::{Digest, Sha256};
use soroban_env_host::{
    budget::Budget,
    e2e_invoke::{
        invoke_host_function, invoke_host_function_in_recording_mode, ledger_entry_to_ledger_key,
        LedgerEntryChange, LedgerEntryLiveUntilChange,
    },
    storage::SnapshotSource,
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

pub fn execute_svm_in_recording_mode(
    enable_diagnostics: bool,
    host_fn: &HostFunction,
    source_account: &AccountId,
    ledger_info: LedgerInfo,
    prng_seed: [u8; 32],
    ledger_snapshot: Rc<dyn SnapshotSource>,
) -> Result<InvokeHostFunctionHelperResult, HostError> {
    let limits = Limits::none();
    let encoded_host_fn = host_fn.to_xdr(limits.clone()).unwrap();
    let encoded_source_account = source_account.to_xdr(limits.clone()).unwrap();

    let budget = Budget::default();

    budget.reset_unlimited()?;

    let mut diagnostic_events = Vec::<DiagnosticEvent>::new();
    let res = invoke_host_function_in_recording_mode(
        &budget,
        enable_diagnostics,
        host_fn,
        source_account,
        None,
        ledger_info,
        ledger_snapshot,
        prng_seed,
        &mut diagnostic_events,
    )?;

    Ok(InvokeHostFunctionHelperResult {
        invoke_result: res.invoke_result,
        ledger_changes: res.ledger_changes.into_iter().map(|c| c.into()).collect(),
        contract_events: res.contract_events,
        diagnostic_events,
        budget,
        retroshades: res.retroshades,
    })
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

#[cfg(test)]
mod test {
    use soroban_env_host::{
        xdr::{AccountId, HostFunction, PublicKey, Uint256},
        LedgerInfo,
    };

    use super::execute_svm;

    #[test]
    fn execute_mainnet() {
        let mut ledger_info = LedgerInfo::default();
        ledger_info.protocol_version = 22;

        let execution = execute_svm(
            true,
            &serde_json::from_str(
                r#"{"invoke_contract":{"contract_address":"CB6WUNOICTMDMEBS7E7AGC3MEN43UA53QT4OT355F22VWMLOUJWWKMHH","function_name":"t","args":[]}}"#,
            ).unwrap(),
            &serde_json::from_str(
                r#"{"footprint":{"read_only":[{"contract_data":{"contract":"CB6WUNOICTMDMEBS7E7AGC3MEN43UA53QT4OT355F22VWMLOUJWWKMHH","key":"ledger_key_contract_instance","durability":"persistent"}},{"contract_code":{"hash":"5bf30f4ebf6e399a0f6cf8c7d134f2e6741ab78455aa6bcb20e3dc01261ea5e3"}}],"read_write":[]},"instructions":492586,"read_bytes":864,"write_bytes":80000}"#,
            ).unwrap(),
            &AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([0;32]))),
            vec![],
            &ledger_info,
            serde_json::from_str(r#"[[{"last_modified_ledger_seq":1470890,"data":{"contract_data":{"ext":"v0","contract":"CB6WUNOICTMDMEBS7E7AGC3MEN43UA53QT4OT355F22VWMLOUJWWKMHH","key":"ledger_key_contract_instance","durability":"persistent","val":{"contract_instance":{"executable":{"wasm":"5bf30f4ebf6e399a0f6cf8c7d134f2e6741ab78455aa6bcb20e3dc01261ea5e3"},"storage":null}}}},"ext":"v0"},3544489],[{"last_modified_ledger_seq":1470885,"data":{"contract_code":{"ext":{"v1":{"ext":"v0","cost_inputs":{"ext":"v0","n_instructions":3,"n_functions":2,"n_globals":3,"n_table_entries":0,"n_types":2,"n_data_segments":0,"n_elem_segments":0,"n_imports":0,"n_exports":5,"n_data_segment_bytes":0}}},"hash":"5bf30f4ebf6e399a0f6cf8c7d134f2e6741ab78455aa6bcb20e3dc01261ea5e3","code":"0061736d010000000115046000017e60037e7e7e017e60027e7e017e600000021303017801370000016d01390001017801390002030302000305030100110619037f01418080c0000b7f00418c80c0000b7f00419080c0000b072d05066d656d6f7279020001740003015f00040a5f5f646174615f656e6403010b5f5f686561705f6261736503020a64025f01017f23808080800041106b22002480808080002000108080808000370308428ef2b8b50e418480c08000ad422086420484200041086aad4220864204844284808080101081808080001082808080001a200041106a24808080800042020b02000b0b150100418080c0000b0c74657374000010000400000000630e636f6e747261637473706563763000000001000000000000000000000000f46697273745265747269736861646500000000010000000000000004746573740000001300000000000000000000000174000000000000000000000100000365000000000020e636f6e7472616374656e766d6574617630000000000000001500000000006f0e636f6e74726163746d65746176300000000000000005727376657200000000000006312e38302e3100000000000000000008727373646b766572000000002f32312e342e30236436663536333966363433643736653735386265656362623063613339316638636433303463323400"}},"ext":"v0"},3544484]]"#).unwrap(),
            &[0;32],
        );

        println!("{:?}", execution)
    }
}
