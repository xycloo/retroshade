//! Notes on this test.
//! Notes on this test.
//! The `storage` contract holds logic so that `if 1_i128 == env.storage().instance().get::<i32, i128>(&0).unwrap()` then
//! the retroshade event is emitted, and then updates the instance so that 0 -> 2_i128 (from 0 -> 1_i128).
//!
//! To make a real test resembling chain state, the snapshotsource is returning the updated state (it gets executed after the tx is applied)
//!  0 -> 2_i128 which wont trigger the retroshade event. However, the txmeta of the transaction notices that the instance entry was updated
//! and resets the state of the svm fork execution to 0 -> 1_i128, allowing the retroshade emission code to be reached.
//!

use std::collections::HashMap;

use crate::{
    conversion::{FromScVal, TypeKind},
    PackedEventEntry, RetroshadeExportPretty, RetroshadesExecution,
};
use postgres_types::Type;
use soroban_env_host::{
    storage::SnapshotSource,
    xdr::{
        ContractCodeEntry, ContractDataEntry, ContractExecutable, ExtensionPoint, Hash,
        HostFunction, Int128Parts, InvokeContractArgs, InvokeHostFunctionOp, LedgerEntry,
        LedgerEntryChange, LedgerEntryChanges, LedgerEntryData, LedgerEntryExt, LedgerFootprint,
        LedgerKey, LedgerKeyContractCode, LedgerKeyContractData, MuxedAccount, Operation,
        OperationBody, OperationMeta, ScAddress, ScContractInstance, ScMap, ScMapEntry, ScSymbol,
        ScVal, ScVec, SequenceNumber, SorobanResources, SorobanTransactionDataExt,
        SorobanTransactionMeta, Transaction, TransactionMetaV3, TransactionV1Envelope, Uint256,
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
                    code: std::fs::read("/Users/tdep/projects/retroshade/examples/storage/target/wasm32-unknown-unknown/release/soroban_hello_world_contract.wasm").unwrap().try_into().unwrap()
                })
            },
            LedgerKey::ContractData(_) => LedgerEntry { last_modified_ledger_seq: 0, data: LedgerEntryData::ContractData(ContractDataEntry {
                ext: ExtensionPoint::V0,
                contract: ScAddress::Contract(Hash([0;32]).into()),
                durability: soroban_env_host::xdr::ContractDataDurability::Persistent,
                key: soroban_env_host::xdr::ScVal::LedgerKeyContractInstance,
                val: ScVal::ContractInstance(ScContractInstance {
                    executable: ContractExecutable::Wasm(Hash([0; 32])),
                    storage: Some(ScMap(vec![ScMapEntry {
                        key: ScVal::I32(0),
                        val: ScVal::I128(Int128Parts {hi: 0, lo: 2})
                    }].try_into().unwrap()))
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

    let _put_envelope = TransactionV1Envelope {
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
                            read_only: vec![LedgerKey::ContractCode(LedgerKeyContractCode {
                                hash: Hash([0; 32]),
                            })]
                            .try_into()
                            .unwrap(),
                            read_write: vec![LedgerKey::ContractData(LedgerKeyContractData {
                                contract: ScAddress::Contract(Hash([0; 32]).into()),
                                key: ScVal::LedgerKeyContractInstance,
                                durability:
                                    soroban_env_host::xdr::ContractDataDurability::Persistent,
                            })]
                            .try_into()
                            .unwrap(),
                        },
                        instructions: 10000000,
                        disk_read_bytes: 1000000,
                        write_bytes: 100000,
                    },
                    resource_fee: 10000000,
                },
            ),
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                    host_function: HostFunction::InvokeContract(InvokeContractArgs {
                        contract_address: ScAddress::Contract(Hash([0; 32]).into()),
                        function_name: ScSymbol("put".try_into().unwrap()),
                        args: vec![].try_into().unwrap(),
                    }),
                    auth: vec![].try_into().unwrap(),
                }),
            }]
            .try_into()
            .unwrap(),
        },
    };

    let t_envelope = TransactionV1Envelope {
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
                            read_only: vec![LedgerKey::ContractCode(LedgerKeyContractCode {
                                hash: Hash([0; 32]),
                            })]
                            .try_into()
                            .unwrap(),
                            read_write: vec![LedgerKey::ContractData(LedgerKeyContractData {
                                contract: ScAddress::Contract(Hash([0; 32]).into()),
                                key: ScVal::LedgerKeyContractInstance,
                                durability:
                                    soroban_env_host::xdr::ContractDataDurability::Persistent,
                            })]
                            .try_into()
                            .unwrap(),
                        },
                        instructions: 10000000,
                        disk_read_bytes: 1000000,
                        write_bytes: 100000,
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
            return_value: ScVal::Vec(Some(ScVec(vec![].try_into().unwrap()))),
            diagnostic_events: vec![].try_into().unwrap(),
        }),
        operations: vec![OperationMeta {
            changes: LedgerEntryChanges(
                vec![
                    LedgerEntryChange::State(LedgerEntry {
                        last_modified_ledger_seq: 0,
                        data: LedgerEntryData::ContractData(ContractDataEntry {
                            ext: ExtensionPoint::V0,
                            contract: ScAddress::Contract(Hash([0; 32]).into()),
                            durability: soroban_env_host::xdr::ContractDataDurability::Persistent,
                            key: soroban_env_host::xdr::ScVal::LedgerKeyContractInstance,
                            val: ScVal::ContractInstance(ScContractInstance {
                                executable: ContractExecutable::Wasm(Hash([0; 32])),
                                storage: Some(ScMap(
                                    vec![ScMapEntry {
                                        key: ScVal::I32(0),
                                        val: ScVal::I128(Int128Parts { hi: 0, lo: 1 }),
                                    }]
                                    .try_into()
                                    .unwrap(),
                                )),
                            }),
                        }),
                        ext: LedgerEntryExt::V0,
                    }),
                    LedgerEntryChange::Updated(LedgerEntry {
                        last_modified_ledger_seq: 0,
                        data: LedgerEntryData::ContractData(ContractDataEntry {
                            ext: ExtensionPoint::V0,
                            contract: ScAddress::Contract(Hash([0; 32]).into()),
                            durability: soroban_env_host::xdr::ContractDataDurability::Persistent,
                            key: soroban_env_host::xdr::ScVal::LedgerKeyContractInstance,
                            val: ScVal::ContractInstance(ScContractInstance {
                                executable: ContractExecutable::Wasm(Hash([0; 32])),
                                storage: Some(ScMap(
                                    vec![ScMapEntry {
                                        key: ScVal::I32(0),
                                        val: ScVal::I128(Int128Parts { hi: 0, lo: 2 }),
                                    }]
                                    .try_into()
                                    .unwrap(),
                                )),
                            }),
                        }),
                        ext: LedgerEntryExt::V0,
                    }),
                ]
                .try_into()
                .unwrap(),
            ),
        }]
        .try_into()
        .unwrap(),
    };

    retroshades
        .build_from_envelope_and_meta(Box::new(snapshot_source), t_envelope, meta, HashMap::new())
        .unwrap();

    let retroshades_result = retroshades.retroshade().unwrap();

    // println!("{:?}", retroshades_result.diagnostic);

    assert_eq!(
        "[{\"contract_id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"target\":{\"symbol\":\"test\"},\"event_object\":{\"map\":[{\"key\":{\"symbol\":\"amount\"},\"val\":{\"i128\":{\"hi\":0,\"lo\":2}}},{\"key\":{\"symbol\":\"test\"},\"val\":{\"address\":\"CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4\"}}]}}]",
        serde_json::to_string(&retroshades_result.retroshades).unwrap()
    );

    let retroshades_pretty = retroshades.retroshade_packed().unwrap();

    assert_eq!(
        retroshades_pretty.retroshades,
        vec![RetroshadeExportPretty {
            contract_id: "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4".to_string(),
            target: "test".to_string(),
            event: vec![
                PackedEventEntry {
                    name: "amount".to_string(),
                    value: FromScVal {
                        dbtype: Type::NUMERIC,
                        kind: TypeKind::Numeric("2".to_string())
                    }
                },
                PackedEventEntry {
                    name: "test".to_string(),
                    value: FromScVal {
                        dbtype: Type::TEXT,
                        kind: TypeKind::Text(
                            "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4".to_string()
                        )
                    }
                }
            ]
        }]
    );
}
