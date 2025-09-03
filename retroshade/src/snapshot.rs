use std::rc::Rc;

use soroban_env_host::{
    storage::SnapshotSource,
    xdr::{
        LedgerEntry, LedgerEntryData, LedgerKey, LedgerKeyAccount, LedgerKeyContractCode,
        LedgerKeyContractData, LedgerKeyTrustLine,
    },
};

pub struct InternalSnapshot {
    inner_source: Rc<dyn SnapshotSource>,
    target_pre_execution_state: Vec<(LedgerEntry, Option<u32>)>,
    force_remove: Vec<LedgerEntry>,
}

impl InternalSnapshot {
    pub(crate) fn new(
        inner_source: Rc<dyn SnapshotSource>,
        target_pre_execution_state: Vec<(LedgerEntry, Option<u32>)>,
        force_remove: Vec<LedgerEntry>,
    ) -> Self {
        Self {
            inner_source,
            target_pre_execution_state,
            force_remove,
        }
    }
}

impl SnapshotSource for InternalSnapshot {
    fn get(
        &self,
        key: &Rc<soroban_env_host::xdr::LedgerKey>,
    ) -> Result<Option<soroban_env_host::storage::EntryWithLiveUntil>, soroban_env_host::HostError>
    {
        if let Some((entry, lifetime)) =
            self.target_pre_execution_state.iter().find(|(entry, _)| {
                let entry_key = match &entry.data {
                    LedgerEntryData::Account(account) => LedgerKey::Account(LedgerKeyAccount {
                        account_id: account.account_id.clone(),
                    }),
                    LedgerEntryData::ContractCode(code) => {
                        LedgerKey::ContractCode(LedgerKeyContractCode {
                            hash: code.hash.clone(),
                        })
                    }
                    LedgerEntryData::ContractData(data) => {
                        LedgerKey::ContractData(LedgerKeyContractData {
                            contract: data.contract.clone(),
                            key: data.key.clone(),
                            durability: data.durability,
                        })
                    }
                    LedgerEntryData::Trustline(trustline) => {
                        LedgerKey::Trustline(LedgerKeyTrustLine {
                            asset: trustline.asset.clone(),
                            account_id: trustline.account_id.clone(),
                        })
                    }
                    _ => return false,
                };
                key.as_ref() == &entry_key
            })
        {
            return Ok(Some((Rc::new(entry.clone()), *lifetime)));
        }

        if self
            .force_remove
            .iter()
            .find(|entry| {
                let entry_key = match &entry.data {
                    LedgerEntryData::Account(account) => LedgerKey::Account(LedgerKeyAccount {
                        account_id: account.account_id.clone(),
                    }),
                    LedgerEntryData::ContractCode(code) => {
                        LedgerKey::ContractCode(LedgerKeyContractCode {
                            hash: code.hash.clone(),
                        })
                    }
                    LedgerEntryData::ContractData(data) => {
                        LedgerKey::ContractData(LedgerKeyContractData {
                            contract: data.contract.clone(),
                            key: data.key.clone(),
                            durability: data.durability,
                        })
                    }
                    LedgerEntryData::Trustline(trustline) => {
                        LedgerKey::Trustline(LedgerKeyTrustLine {
                            asset: trustline.asset.clone(),
                            account_id: trustline.account_id.clone(),
                        })
                    }
                    _ => return false,
                };

                key.as_ref() == &entry_key
            })
            .is_some()
        {
            return Ok(None);
        }

        self.inner_source.get(key)
    }
}
