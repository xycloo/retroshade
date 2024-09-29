use std::rc::Rc;

use soroban_env_host::{
    storage::SnapshotSource,
    xdr::{
        LedgerEntry, LedgerEntryData, LedgerKey, LedgerKeyAccount, LedgerKeyConfigSetting,
        LedgerKeyContractCode, LedgerKeyContractData, LedgerKeyTrustLine,
    },
};

pub struct InternalSnapshot {
    inner_source: Rc<dyn SnapshotSource>,
    target_pre_execution_state: Vec<(LedgerEntry, Option<u32>)>,
}

impl InternalSnapshot {
    pub(crate) fn new(
        inner_source: Rc<dyn SnapshotSource>,
        target_pre_execution_state: Vec<(LedgerEntry, Option<u32>)>,
    ) -> Self {
        Self {
            inner_source,
            target_pre_execution_state,
        }
    }
}

impl SnapshotSource for InternalSnapshot {
    fn get(
        &self,
        key: &Rc<soroban_env_host::xdr::LedgerKey>,
    ) -> Result<Option<soroban_env_host::storage::EntryWithLiveUntil>, soroban_env_host::HostError>
    {
        println!("requesting entry {:?}", key);
        for (entry, _) in &self.target_pre_execution_state {
            let entry_key = match &entry.data {
                LedgerEntryData::Account(account) => LedgerKey::Account(LedgerKeyAccount {
                    account_id: account.account_id.clone(),
                }),
                LedgerEntryData::ContractCode(code) => {
                    println!("got code for pre_execution");
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
                LedgerEntryData::Trustline(trustline) => LedgerKey::Trustline(LedgerKeyTrustLine {
                    asset: trustline.asset.clone(),
                    account_id: trustline.account_id.clone(),
                }),

                // Some ledger key that will never be matched
                _ => LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
                    config_setting_id: soroban_env_host::xdr::ConfigSettingId::StateArchival,
                }),
            };

            if key.as_ref() == &entry_key {
                return Ok(Some((Rc::new(entry.clone()), None)));
            }
        }

        self.inner_source.get(key)
    }
}
