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
        AccountId, ContractCodeEntry, ContractDataEntry, ContractExecutable, ExtensionPoint, Hash,
        HostFunction, Int128Parts, InvokeContractArgs, InvokeHostFunctionOp, LedgerEntry,
        LedgerEntryChange, LedgerEntryChanges, LedgerEntryData, LedgerEntryExt, LedgerFootprint,
        LedgerKey, LedgerKeyContractCode, LedgerKeyContractData, MuxedAccount, Operation,
        OperationBody, OperationMeta, PublicKey, ScAddress, ScContractInstance, ScMap, ScMapEntry,
        ScSymbol, ScVal, ScVec, SequenceNumber, SorobanResources, SorobanTransactionMeta,
        Transaction, TransactionMetaV3, TransactionV1Envelope, Uint256,
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
                    code: std::fs::read("/Users/tdep/projects/retroshade/examples/soroban-insurance-factory/target/wasm32-unknown-unknown/release/soroban_hello_world_contract.wasm").unwrap().try_into().unwrap()
                })
            },
            LedgerKey::ContractData(_) => LedgerEntry { last_modified_ledger_seq: 0, data: LedgerEntryData::ContractData(ContractDataEntry {
                ext: ExtensionPoint::V0,
                contract: ScAddress::Contract(Hash([0;32])),
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
                    ext: ExtensionPoint::V0,
                    resources: SorobanResources {
                        footprint: LedgerFootprint {
                            read_only: vec![LedgerKey::ContractCode(LedgerKeyContractCode {
                                hash: Hash([0; 32]),
                            })]
                            .try_into()
                            .unwrap(),
                            read_write: vec![LedgerKey::ContractData(LedgerKeyContractData {
                                contract: ScAddress::Contract(Hash([0; 32])),
                                key: ScVal::LedgerKeyContractInstance,
                                durability:
                                    soroban_env_host::xdr::ContractDataDurability::Persistent,
                            })]
                            .try_into()
                            .unwrap(),
                        },
                        instructions: 10000000,
                        read_bytes: 1000000,
                        write_bytes: 100000,
                    },
                    resource_fee: 10000000,
                },
            ),
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                    host_function: HostFunction::InvokeContract(InvokeContractArgs {
                        contract_address: ScAddress::Contract(Hash([0; 32])),
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
                    ext: ExtensionPoint::V0,
                    resources: SorobanResources {
                        footprint: LedgerFootprint {
                            read_only: vec![LedgerKey::ContractCode(LedgerKeyContractCode {
                                hash: Hash([0; 32]),
                            })]
                            .try_into()
                            .unwrap(),
                            read_write: vec![LedgerKey::ContractData(LedgerKeyContractData {
                                contract: ScAddress::Contract(Hash([0; 32])),
                                key: ScVal::LedgerKeyContractInstance,
                                durability:
                                    soroban_env_host::xdr::ContractDataDurability::Persistent,
                            })]
                            .try_into()
                            .unwrap(),
                        },
                        instructions: 10000000,
                        read_bytes: 1000000,
                        write_bytes: 100000,
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
                            contract: ScAddress::Contract(Hash([0; 32])),
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
                            contract: ScAddress::Contract(Hash([0; 32])),
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

/*

{"invoke_contract":{"contract_address":"CB5EORUHJO4VNEP4EM533NGALJASPRUHTQBSF7UAU4MQCFE2JECULYIF","function_name":"initialize","args":[{"address":"GAT4ALGIWVMXQZLWQ
7ESOU55SXX5ELZRDTSVVHMJWIMCDHQMHZY43WM6"},{"bytes":"0000000000000000000000000000000000000000000000000000000034426326"},{"address":"CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIX
F47ZG2FB2RMQQVU2HHGCYSC"},{"address":"CCYOZJCOPG34LLQQ7N24YXBM7LL62R7ONMZ3G6WZAAYPB5OYKOMJRN63"},{"symbol":"XLM"},{"bool":true},"void",{"i32":2},{"i128":{"hi":0,"lo
":100000}},{"i32":140}]}}
{"footprint":{"read_only":[{"contract_data":{"contract":"CB5EORUHJO4VNEP4EM533NGALJASPRUHTQBSF7UAU4MQCFE2JECULYIF","key":"ledger_key_contract_instance","durability"
:"persistent"}},{"contract_code":{"hash":"379a5e8b34e61a7a93e1e05b5aaab4e1ad223f1c9e36cbc315579d24ae40199f"}}],"read_write":[]},"instructions":782675,"read_bytes":1
996,"write_bytes":0}
[[{"last_modified_ledger_seq":145197,"data":{"contract_data":{"ext":"v0","contract":"CB5EORUHJO4VNEP4EM533NGALJASPRUHTQBSF7UAU4MQCFE2JECULYIF","key":"ledger_key_con
tract_instance","durability":"persistent","val":{"contract_instance":{"executable":{"wasm":"379a5e8b34e61a7a93e1e05b5aaab4e1ad223f1c9e36cbc315579d24ae40199f"},"stor
age":[{"key":{"vec":[{"symbol":"PoolHash"}]},"val":{"bytes":"9daaac32b5585824d7c593a59b549b67dc05c65c47233f4e6629655fac9ee188"}}]}}}},"ext":"v0"},2218755],[{"last_m
odified_ledger_seq":145154,"data":{"contract_code":{"ext":{"v1":{"ext":"v0","cost_inputs":{"ext":"v0","n_instructions":329,"n_functions":9,"n_globals":3,"n_table_en
tries":0,"n_types":8,"n_data_segments":1,"n_elem_segments":0,"n_imports":8,"n_exports":6,"n_data_segment_bytes":8}}},"hash":"379a5e8b34e61a7a93e1e05b5aaab4e1ad223f1
c9e36cbc315579d24ae40199f","code":"0061736d0100000001380960027e7e017e60037e7e7e017e60017e017e6000017e60027f7f017e60027f7e0060017e017f600a7e7e7e7e7e7e7e7e7e7e017e600
00002490c017601670000016c015f0001016901380002016901370002016c01310000017801370003016c01330001016d013900010178013900000162016a0000016201380002016c01300000030b0a03040
20506070808080805030100110619037f01418080c0000b7f0041a480c0000b7f0041b080c0000b074506066d656d6f727902000c696e69745f666163746f7279000e0a696e697469616c697a650011015f0
0150a5f5f646174615f656e6403010b5f5f686561705f6261736503020af0060a5102017f017e23808080800041106b22002480808080002000418080c080004108108d80808000370308200041086aad422
0864204844284808080101080808080002101200041106a24808080800020010bc60102017e047f0240200141094b0d00420021022001210320002104024003402003450d0141012105024020042d0000220
641df00460d000240200641506a41ff0171410a490d000240200641bf7f6a41ff0171411a490d002006419f7f6a41ff017141194b0d05200641456a21050c020b2006414b6a21050c010b200641526a21050
b20024206862005ad42ff01838421022003417f6a2103200441016a21040c000b0b2002420886420e840f0b2000ad4220864204842001ad4220864204841089808080000b6c02017f017e238080808000411
06b220124808080800020012000108f80808000024020012802000d0020012903082102420321000240108c808080001090808080000d0042022100108c80808000200242021081808080001a0b200141106
a24808080800020000f0b00000b3f01017e420121020240200142ff018342c800520d002001108a80808000428080808070834280808080800452ad21020b20002001370308200020023703000b0f0020004
202108b808080004201510bfa0201027f23808080800041306b220a248080808000024002400240200042ff018342cd00520d00200a41186a2001108f80808000200a290318a70d00200242ff018342cd00520d00200342ff018342cd00520d00200a290320210102402004a741ff0171220b410e460d00200b41ca00470d010b200542fe018350450d00024020064202510d00200642ff018342cd00520d010b200742ff01834205520d0002402008a741ff0171220b410b460d00200b41c500470d0120081082808080001a20081083808080001a0b200942ff01834205520d00108c808080002208109080808000450d01200a41086a20084202108480808000108f80808000200a290308a7450d020b00000b109280808000000b200a2903102108108580808000200820011086808080001a418880c08000410f108d808080002108200a20003703282008419c80c08000ad422086420484200a41286aad4220864204844284808080101087808080001088808080001a200a41306a24808080800020000b0900109480808000000b040000000b0900109380808000000b02000b0b2d0100418080c0000b24506f6f6c486173686c69717569646974795f706f6f6c73706f6f6c73170010000500000000ef040e636f6e747261637473706563763000000002000000000000000000000007446174614b65790000000001000000000000000000000008506f6f6c48617368000000040000000000000000000000054572726f72000000000000050000000000000012416c7265616479496e697469616c697a6564000000000000000000000000000e4e6f74496e697469616c697a656400000000000100000000000000084e6f7441646d696e00000002000000000000000a506f6f6c45786973747300000000000300000000000000064e6f506f6f6c0000000000040000000100000000000000000000000e4c6971756964697479506f6f6c730000000000010000000000000005706f6f6c730000000000001300000000000000000000000c696e69745f666163746f7279000000010000000000000009706f6f6c5f68617368000000000003ee0000002000000001000003e9000003ed000000000000000300000000000000000000000a696e697469616c697a6500000000000a000000000000000561646d696e00000000000013000000000000000473616c74000003ee000000200000000000000005746f6b656e0000000000001300000000000000066f7261636c65000000000013000000000000000673796d626f6c000000000011000000000000000e65787465726e616c5f6173736574000000000001000000000000000c6f7261636c655f6173736574000003e800000013000000000000000f706572696f64735f696e5f646179730000000005000000000000000a766f6c6174696c69747900000000000b000000000000000a6d756c7469706c69657200000000000500000001000003e90000001300000003001e11636f6e7472616374656e766d6574617630000000000000001500000000006f0e636f6e74726163746d65746176300000000000000005727376657200000000000006312e38312e3000000000000000000008727373646b7665720000002f32312e342e30236436663536333966363433643736653735386265656362623063613339316638636433303463323400"}},"ext":"v0"},2218753]]

*/

#[test]
fn test_initialize_function() {
    let mut retroshades = RetroshadesExecution::new(LedgerInfo {
        protocol_version: 22,
        sequence_number: 145197,
        timestamp: 200,
        network_id: [0; 32],
        base_reserve: 1,
        min_temp_entry_ttl: 300,
        min_persistent_entry_ttl: 400,
        max_entry_ttl: 500000,
    });

    #[derive(Clone)]
    struct TestSnapshot;

    impl SnapshotSource for TestSnapshot {
        fn get(
            &self,
            key: &std::rc::Rc<soroban_env_host::xdr::LedgerKey>,
        ) -> Result<
            Option<soroban_env_host::storage::EntryWithLiveUntil>,
            soroban_env_host::HostError,
        > {
            match key.as_ref() {
                LedgerKey::ContractData(_) => {
                    let entry = LedgerEntry {
                        last_modified_ledger_seq: 145197,
                        data: LedgerEntryData::ContractData(ContractDataEntry {
                            ext: ExtensionPoint::V0,
                            contract: ScAddress::Contract(Hash([0; 32])),
                            key: ScVal::LedgerKeyContractInstance,
                            val: ScVal::ContractInstance(ScContractInstance {
                                executable: ContractExecutable::Wasm(Hash([0; 32])),
                                storage: Some(ScMap(
                                    vec![ScMapEntry {
                                        key: ScVal::Vec(Some(ScVec(
                                            vec![ScVal::Symbol("PoolHash".try_into().unwrap())]
                                                .try_into()
                                                .unwrap(),
                                        ))),
                                        val: ScVal::Bytes([0; 32].to_vec().try_into().unwrap()),
                                    }]
                                    .try_into()
                                    .unwrap(),
                                )),
                            }),
                            durability: soroban_env_host::xdr::ContractDataDurability::Persistent,
                        }),
                        ext: LedgerEntryExt::V0,
                    };
                    Ok(Some((Rc::new(entry), Some(2218755))))
                }
                LedgerKey::ContractCode(_) => {
                    let entry = LedgerEntry {
                        last_modified_ledger_seq: 145154,
                        data: LedgerEntryData::ContractCode(ContractCodeEntry {
                            ext: soroban_env_host::xdr::ContractCodeEntryExt::V0,
                            hash: Hash([
                                0;32]),
                            code: std::fs::read("/Users/tdep/projects/retroshade/examples/soroban-insurance-factory/target/wasm32-unknown-unknown/release/soroban_hello_world_contract.wasm").unwrap().try_into().unwrap()
                        }),
                        ext: LedgerEntryExt::V0,
                    };
                    Ok(Some((Rc::new(entry), Some(2218753))))
                }
                _ => Ok(None),
            }
        }
    }

    let snapshot_source = TestSnapshot;

    let t_envelope = TransactionV1Envelope {
        tx: Transaction {
            source_account: MuxedAccount::Ed25519(Uint256([0; 32])),
            fee: 0,
            seq_num: SequenceNumber(1),
            cond: soroban_env_host::xdr::Preconditions::None,
            memo: soroban_env_host::xdr::Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                    host_function: HostFunction::InvokeContract(InvokeContractArgs {
                        contract_address: ScAddress::Contract(Hash([0; 32])),
                        function_name: ScSymbol("initialize".try_into().unwrap()),
                        args: vec![
                            ScVal::Address(ScAddress::Account(AccountId(
                                PublicKey::PublicKeyTypeEd25519(Uint256([0; 32])),
                            ))),
                            ScVal::Bytes(
                                vec![
                                    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x34, 0x42,
                                    0x63, 0x26,
                                ]
                                .try_into()
                                .unwrap(),
                            ),
                            ScVal::Address(ScAddress::Account(AccountId(
                                PublicKey::PublicKeyTypeEd25519(Uint256([0; 32])),
                            ))),
                            ScVal::Address(ScAddress::Account(AccountId(
                                PublicKey::PublicKeyTypeEd25519(Uint256([0; 32])),
                            ))),
                            ScVal::Symbol("XLM".try_into().unwrap()),
                            ScVal::Bool(true),
                            ScVal::Void,
                            ScVal::I32(2),
                            ScVal::I128(Int128Parts { hi: 0, lo: 100000 }),
                            ScVal::I32(140),
                        ]
                        .try_into()
                        .unwrap(),
                    }),
                    auth: vec![].try_into().unwrap(),
                }),
            }]
            .try_into()
            .unwrap(),
            ext: soroban_env_host::xdr::TransactionExt::V1(
                soroban_env_host::xdr::SorobanTransactionData {
                    ext: ExtensionPoint::V0,
                    resources: SorobanResources {
                        footprint: LedgerFootprint {
                            read_only: vec![
                                LedgerKey::ContractData(LedgerKeyContractData {
                                    contract: ScAddress::Contract(Hash([0; 32])),
                                    key: ScVal::LedgerKeyContractInstance,
                                    durability:
                                        soroban_env_host::xdr::ContractDataDurability::Persistent,
                                }),
                                LedgerKey::ContractCode(LedgerKeyContractCode {
                                    hash: Hash([0; 32]),
                                }),
                            ]
                            .try_into()
                            .unwrap(),
                            read_write: vec![].try_into().unwrap(),
                        },
                        instructions: 7800075,
                        read_bytes: 19900006,
                        write_bytes: 1000000,
                    },
                    resource_fee: 100000000,
                },
            ),
        },
        signatures: vec![].try_into().unwrap(),
    };

    let meta = TransactionMetaV3 {
        ext: ExtensionPoint::V0,
        tx_changes_before: LedgerEntryChanges(vec![].try_into().unwrap()),
        tx_changes_after: LedgerEntryChanges(vec![].try_into().unwrap()),
        operations: vec![].try_into().unwrap(),
        soroban_meta: Some(SorobanTransactionMeta {
            ext: soroban_env_host::xdr::SorobanTransactionMetaExt::V0,
            events: vec![].try_into().unwrap(),
            return_value: ScVal::Void,
            diagnostic_events: vec![].try_into().unwrap(),
        }),
    };

    retroshades
        .build_from_envelope_and_meta(
            Box::new(snapshot_source.clone()),
            t_envelope,
            meta,
            HashMap::new(),
        )
        .unwrap();

    let retroshades_result = retroshades
        .retroshade_recording(Rc::new(snapshot_source))
        .unwrap();

    // Add assertions here to check the expected outcome
    // For example:
    // assert_eq!(retroshades_result.retroshades.len(), 1);
    // assert_eq!(retroshades_result.retroshades[0].contract_id, "CB5EORUHJO4VNEP4EM533NGALJASPRUHTQBSF7UAU4MQCFE2JECULYIF");
    // assert_eq!(retroshades_result.retroshades[0].target, "initialize");
    // ... more specific assertions based on the expected behavior of the initialize function
}
