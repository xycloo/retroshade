use std::rc::Rc;

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
