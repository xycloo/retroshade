use std::{collections::HashMap, rc::Rc};

use retroshade::RetroshadesExecution;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use soroban_env_host::{
    storage::{EntryWithLiveUntil, SnapshotSource},
    xdr::{
        AccountEntry, ContractCodeEntry, ContractDataEntry, ContractExecutable, ExtensionPoint,
        Hash, HostFunction, InvokeContractArgs, InvokeHostFunctionOp, LedgerEntry,
        LedgerEntryChanges, LedgerEntryData, LedgerEntryExt, LedgerFootprint, LedgerKey,
        LedgerKeyContractCode, LedgerKeyContractData, Limits, MuxedAccount, Operation,
        OperationBody, OperationMeta, PublicKey, ReadXdr, ScAddress, ScContractInstance, ScMap,
        ScSymbol, ScVal, ScVec, SequenceNumber, SorobanResources, SorobanTransactionMeta,
        Thresholds, Transaction, TransactionMetaV3, TransactionV1Envelope, Uint256, WriteXdr,
    },
    LedgerInfo,
};

pub fn get_current_ledger_sequence() -> (i32, i64) {
    let conn = Connection::open("/tmp/rs_ingestion_temp/stellar.db").unwrap();
    let query_string =
        format!("SELECT ledgerseq, closetime FROM ledgerheaders ORDER BY ledgerseq DESC LIMIT 1");

    let mut stmt = conn.prepare(&query_string).unwrap();
    let mut entries = stmt.query(params![]).unwrap();

    let row = entries.next().unwrap();

    if row.is_none() {
        // Unrecoverable: no ledger is running
        return (0, 0);
    }

    (
        row.unwrap().get(0).unwrap_or(0),
        row.unwrap().get(1).unwrap_or(0),
    )
}

pub fn get_ttl(key: LedgerKey) -> u32 {
    let mut hasher = Sha256::new();
    hasher.update(key.to_xdr(Limits::none()).unwrap());
    let result = {
        let hashed = hasher.finalize().as_slice().try_into().unwrap();
        Hash(hashed).to_xdr_base64(Limits::none()).unwrap()
    };

    let conn = Connection::open("/tmp/rs_ingestion_temp/stellar.db").unwrap();
    let query_string = format!("SELECT ledgerentry FROM ttl WHERE keyhash = ?1");

    let mut stmt = conn.prepare(&query_string).unwrap();
    let mut entries = stmt.query(params![result]).unwrap();

    let row = entries.next().unwrap();

    if row.is_none() {
        // TODO: error log
        return 0;
    }

    let entry = {
        let string: String = row.unwrap().get(0).unwrap();
        LedgerEntry::from_xdr_base64(&string, Limits::none()).unwrap()
    };

    let LedgerEntryData::Ttl(ttl) = entry.data else {
        return 0;
    };
    ttl.live_until_ledger_seq
}

pub struct DynamicSnapshot {}

impl SnapshotSource for DynamicSnapshot {
    fn get(
        &self,
        key: &std::rc::Rc<soroban_env_host::xdr::LedgerKey>,
    ) -> Result<Option<soroban_env_host::storage::EntryWithLiveUntil>, soroban_env_host::HostError>
    {
        let entry: Option<EntryWithLiveUntil> = match key.as_ref() {
            LedgerKey::Trustline(trustline) => {
                let PublicKey::PublicKeyTypeEd25519(Uint256(bytes)) = trustline.account_id.0;
                let account_id = stellar_strkey::ed25519::PublicKey(bytes).to_string();
                let asset_xdr = trustline.asset.to_xdr_base64(Limits::none()).unwrap();

                let conn = Connection::open("/tmp/rs_ingestion_temp/stellar.db").unwrap();
                let query_string = format!(
                    "SELECT ledgerentry FROM trustlines where accountid = ?1 AND asset = ?2"
                );

                let mut stmt = conn.prepare(&query_string).unwrap();
                let mut entries = stmt.query(params![account_id, asset_xdr]).unwrap();

                let row = entries.next().unwrap();

                if row.is_none() {
                    return Ok(None);
                }
                let row = row.unwrap();

                let xdr_entry: String = row.get(0).unwrap();
                let xdr_entry = LedgerEntry::from_xdr_base64(xdr_entry, Limits::none()).unwrap();

                Some((Rc::new(xdr_entry), None))
            }

            LedgerKey::Account(key) => {
                let PublicKey::PublicKeyTypeEd25519(ed25519) = key.account_id.0.clone();
                let id = stellar_strkey::ed25519::PublicKey(ed25519.0).to_string();

                let conn = Connection::open("/tmp/rs_ingestion_temp/stellar.db").unwrap();
                let query_string = format!("SELECT balance FROM accounts where accountid = ?1");

                let mut stmt = conn.prepare(&query_string).unwrap();
                let mut entries = stmt.query(params![id]).unwrap();

                let row = entries.next().unwrap();

                if row.is_none() {
                    return Ok(None);
                }
                let row = row.unwrap();

                let entry = LedgerEntry {
                    last_modified_ledger_seq: 0,
                    ext: LedgerEntryExt::V0,
                    data: soroban_env_host::xdr::LedgerEntryData::Account(AccountEntry {
                        account_id: key.account_id.clone(),
                        balance: row.get(0).unwrap(),
                        seq_num: SequenceNumber(0),
                        num_sub_entries: 0,
                        inflation_dest: None,
                        flags: 0,
                        home_domain: Default::default(),
                        thresholds: Thresholds([0; 4]),
                        signers: vec![].try_into().unwrap(),
                        ext: soroban_env_host::xdr::AccountEntryExt::V0,
                    }),
                };

                Some((Rc::new(entry), None))
            }

            LedgerKey::ContractCode(key) => {
                let hash = key.hash.clone();
                let conn = Connection::open("/tmp/rs_ingestion_temp/stellar.db").unwrap();
                let query_string = format!("SELECT ledgerentry FROM contractcode where hash = ?1");

                let mut stmt = conn.prepare(&query_string).unwrap();
                let mut entries = stmt
                    .query(params![hash.to_xdr_base64(Limits::none()).unwrap()])
                    .unwrap();

                let row = entries.next().unwrap();

                if row.is_none() {
                    return Ok(None);
                }
                let row = row.unwrap();

                let xdr_entry: String = row.get(0).unwrap();
                let xdr_entry = LedgerEntry::from_xdr_base64(xdr_entry, Limits::none()).unwrap();

                Some((
                    Rc::new(xdr_entry),
                    Some(get_ttl(LedgerKey::ContractCode(key.clone()))),
                ))
            }

            LedgerKey::ContractData(key) => {
                let contract = key.contract.clone();
                let scval = key.key.clone();

                let conn = Connection::open("/tmp/rs_ingestion_temp/stellar.db").unwrap();
                let query_string = format!(
                    "SELECT ledgerentry FROM contractdata where contractid = ?1 AND key = ?2"
                );

                let mut stmt = conn.prepare(&query_string).unwrap();
                let mut entries = stmt
                    .query(params![
                        contract.to_xdr_base64(Limits::none()).unwrap(),
                        scval.to_xdr_base64(Limits::none()).unwrap()
                    ])
                    .unwrap();
                let row = entries.next().unwrap();

                if row.is_none() {
                    return Ok(None);
                }
                let row = row.unwrap();

                let xdr_entry: String = row.get(0).unwrap();
                let xdr_entry = LedgerEntry::from_xdr_base64(xdr_entry, Limits::none()).unwrap();

                Some((
                    Rc::new(xdr_entry),
                    Some(get_ttl(LedgerKey::ContractData(key.clone()))),
                ))
            }

            _ => None,
        };

        Ok(entry)
    }
}

pub struct TestDynamicSnapshot {}

impl SnapshotSource for TestDynamicSnapshot {
    fn get(
        &self,
        key: &std::rc::Rc<soroban_env_host::xdr::LedgerKey>,
    ) -> Result<Option<soroban_env_host::storage::EntryWithLiveUntil>, soroban_env_host::HostError>
    {
        let entry = match key.as_ref() {
            LedgerKey::ContractCode(_) => LedgerEntry {
                last_modified_ledger_seq: 0,
                ext: LedgerEntryExt::V0,
                data: LedgerEntryData::ContractCode(ContractCodeEntry {
                    ext: soroban_env_host::xdr::ContractCodeEntryExt::V0,
                    hash: Hash([0;32]),
                    code: std::fs::read("/Users/tdep/projects/retroshade/examples/hello_world/target/wasm32-unknown-unknown/release/soroban_hello_world_contract.wasm").unwrap().try_into().unwrap()
                })
            },
            LedgerKey::ContractData(_) => LedgerEntry { last_modified_ledger_seq: 0, data: LedgerEntryData::ContractData(ContractDataEntry {
                ext: ExtensionPoint::V0,
                contract: ScAddress::Contract(Hash([0;32])),
                durability: soroban_env_host::xdr::ContractDataDurability::Persistent,
                key: soroban_env_host::xdr::ScVal::LedgerKeyContractInstance,
                val: ScVal::ContractInstance(ScContractInstance {
                    executable: ContractExecutable::Wasm(Hash([0; 32])),
                    storage: Some(ScMap(vec![].try_into().unwrap()))
                })
            }), ext: LedgerEntryExt::V0 },
            _ => panic!()
        };

        Ok(Some((Rc::new(entry), Some(10000))))
    }
}

fn main() {
    let mut retroshades = RetroshadesExecution::new(LedgerInfo {
        protocol_version: 22,
        sequence_number: 1000,
        timestamp: 200,
        network_id: [0; 32],
        base_reserve: 1,
        min_temp_entry_ttl: 300,
        min_persistent_entry_ttl: 400,
        max_entry_ttl: 500000,
    });

    let snapshot_source = TestDynamicSnapshot {};

    let envelope = TransactionV1Envelope {
        signatures: vec![].try_into().unwrap(),
        tx: Transaction {
            source_account: MuxedAccount::Ed25519(Uint256([0; 32])),
            fee: 0,
            seq_num: SequenceNumber(1),
            cond: soroban_env_host::xdr::Preconditions::None,
            memo: soroban_env_host::xdr::Memo::None,
            ext: soroban_env_host::xdr::TransactionExt::V1(
                soroban_env_host::xdr::SorobanTransactionData {
                    ext: ExtensionPoint::V0,
                    resources: SorobanResources {
                        footprint: LedgerFootprint {
                            read_only: vec![
                                LedgerKey::ContractCode(LedgerKeyContractCode {
                                    hash: Hash([0; 32]),
                                }),
                                LedgerKey::ContractData(LedgerKeyContractData {
                                    contract: ScAddress::Contract(Hash([0; 32])),
                                    key: ScVal::LedgerKeyContractInstance,
                                    durability:
                                        soroban_env_host::xdr::ContractDataDurability::Persistent,
                                }),
                            ]
                            .try_into()
                            .unwrap(),
                            read_write: vec![].try_into().unwrap(),
                        },
                        instructions: 1000000,
                        read_bytes: 10000,
                        write_bytes: 0,
                    },
                    resource_fee: 10000000,
                },
            ),
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                    host_function: HostFunction::InvokeContract(InvokeContractArgs {
                        contract_address: ScAddress::Contract(Hash([0; 32])),
                        function_name: ScSymbol("t".try_into().unwrap()),
                        args: vec![].try_into().unwrap(),
                    }),
                    auth: vec![].try_into().unwrap(),
                }),
            }]
            .try_into()
            .unwrap(),
        },
    };

    let meta = TransactionMetaV3 {
        ext: ExtensionPoint::V0,
        tx_changes_before: LedgerEntryChanges(vec![].try_into().unwrap()),
        tx_changes_after: LedgerEntryChanges(vec![].try_into().unwrap()),
        soroban_meta: Some(SorobanTransactionMeta {
            ext: soroban_env_host::xdr::SorobanTransactionMetaExt::V0,
            events: vec![].try_into().unwrap(),
            return_value: ScVal::Vec(Some(ScVec(
                vec![
                    ScVal::Symbol(ScSymbol("hello".try_into().unwrap())),
                    ScVal::Symbol(ScSymbol("tdep".try_into().unwrap())),
                ]
                .try_into()
                .unwrap(),
            ))),
            diagnostic_events: vec![].try_into().unwrap(),
        }),
        operations: vec![OperationMeta {
            changes: LedgerEntryChanges(vec![].try_into().unwrap()), // the hello world contract doesn't change anything
        }]
        .try_into()
        .unwrap(),
    };

    retroshades
        .build_from_envelope_and_meta(Box::new(snapshot_source), envelope, meta, HashMap::new())
        .unwrap();
    let retroshades = retroshades.retroshade().unwrap();

    println!(
        "{}",
        serde_json::to_string(&retroshades.retroshades).unwrap()
    )
}
