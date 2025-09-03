use std::collections::HashMap;

use crate::RetroshadesExecution;
use soroban_env_host::{
    storage::SnapshotSource,
    xdr::{
        ContractCodeEntry, ContractDataEntry, ContractExecutable, ExtensionPoint, Hash,
        HostFunction, InvokeContractArgs, InvokeHostFunctionOp, LedgerEntry, LedgerEntryChanges,
        LedgerEntryData, LedgerEntryExt, LedgerFootprint, LedgerKey, LedgerKeyContractCode,
        LedgerKeyContractData, MuxedAccount, Operation, OperationBody, OperationMeta, ScAddress,
        ScContractInstance, ScMap, ScSymbol, ScVal, ScVec, SequenceNumber, SorobanResources,
        SorobanTransactionDataExt, SorobanTransactionMeta, Transaction, TransactionMeta,
        TransactionMetaV3, TransactionV1Envelope, Uint256,
    },
    LedgerInfo,
};

use std::rc::Rc;

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
                contract: ScAddress::Contract(Hash([0;32]).into()),
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

#[test]
fn simple() {
    let mut retroshades = RetroshadesExecution::new(LedgerInfo {
        protocol_version: 23,
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
                    ext: SorobanTransactionDataExt::V0,
                    resources: SorobanResources {
                        footprint: LedgerFootprint {
                            read_only: vec![
                                LedgerKey::ContractCode(LedgerKeyContractCode {
                                    hash: Hash([0; 32]),
                                }),
                                LedgerKey::ContractData(LedgerKeyContractData {
                                    contract: ScAddress::Contract(Hash([0; 32]).into()),
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
                        disk_read_bytes: 10000,
                        write_bytes: 0,
                    },
                    resource_fee: 10000000,
                },
            ),
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                    host_function: HostFunction::InvokeContract(InvokeContractArgs {
                        contract_address: ScAddress::Contract(Hash([0; 32]).into()),
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

    let mut mercury_contracts = HashMap::new();
    let binary = std::fs::read("/Users/tdep/projects/retroshade/examples/hello_world/target/wasm32-unknown-unknown/release/soroban_hello_world_contract.wasm").unwrap();
    mercury_contracts.insert(Hash([0; 32]), binary.as_slice());

    let replaced = retroshades
        .build_from_envelope_and_meta(
            Box::new(snapshot_source),
            envelope,
            TransactionMeta::V3(meta),
            mercury_contracts,
        )
        .unwrap();

    assert!(replaced);

    let retroshades = retroshades.retroshade_packed().unwrap();
    println!("{:?}", &retroshades.retroshades);
    /*assert_eq!(
        r#"[{"contract_id":"0000000000000000000000000000000000000000000000000000000000000000","target":{"symbol":"test"},"event_object":{"map":[{"key":{"symbol":"test"},"val":{"address":"CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"}}]}}]"#,
        serde_json::to_string(&retroshades.retroshades).unwrap()
    )*/
}
