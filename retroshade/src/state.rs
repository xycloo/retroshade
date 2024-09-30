use std::{collections::HashMap, rc::Rc};

use soroban_env_host::{
    storage::SnapshotSource,
    xdr::{
        AccountId, ContractExecutable, Hash, LedgerEntry, LedgerEntryChange, LedgerEntryData,
        MuxedAccount, Operation, OperationBody, OperationMeta, PublicKey, ScAddress, ScVal,
        TransactionExt, TransactionMetaV3, TransactionV1Envelope,
    },
};

use crate::{RetroshadeError, RetroshadesExecution};

impl RetroshadesExecution {
    /// Builds the current state for the requested entries and
    /// sets the resources, auth entries, host function and source account.
    pub(crate) fn build_current_state(
        &mut self,
        snapshot_source: Box<dyn SnapshotSource>,
        envelope: TransactionV1Envelope,
    ) -> Result<(), RetroshadeError> {
        let tx_source = envelope.tx.source_account;

        let resources = match envelope.tx.ext {
            TransactionExt::V1(soroban) => soroban.resources,
            TransactionExt::V0 => return Err(RetroshadeError::NotSorobanTx),
        };

        self.resources = Some(resources.clone());

        if let Some(Operation {
            source_account,
            body,
        }) = envelope.tx.operations.get(0)
        {
            if let OperationBody::InvokeHostFunction(host_fn) = body {
                self.auth_entries = host_fn.auth.to_vec();
                self.host_function = Some(host_fn.host_function.clone());

                let muxed_source = source_account.as_ref().unwrap_or(&tx_source);
                let id = match muxed_source {
                    MuxedAccount::Ed25519(uint) => {
                        AccountId(PublicKey::PublicKeyTypeEd25519(uint.clone()))
                    }
                    MuxedAccount::MuxedEd25519(muxed) => {
                        AccountId(PublicKey::PublicKeyTypeEd25519(muxed.ed25519.clone()))
                    }
                };
                self.source_account = Some(id);
            } else {
                return Err(RetroshadeError::NotSorobanTx);
            }
        } else {
            return Err(RetroshadeError::NotSorobanTx);
        };

        let full_footprint = [
            resources.footprint.read_only.to_vec(),
            resources.footprint.read_write.to_vec(),
        ]
        .concat();

        for key in full_footprint {
            let entry = snapshot_source
                .get(&Rc::new(key.clone()))
                .map_err(|err| RetroshadeError::SVMHost(err))?
                .ok_or(RetroshadeError::EntryNotFound(key))?;

            self.target_pre_execution_state
                .push((entry.0.as_ref().clone(), entry.1))
        }

        Ok(())
    }

    pub(crate) fn state_reset_to_pre_execution(
        &mut self,
        tx_meta: TransactionMetaV3,
    ) -> Result<bool, RetroshadeError> {
        let mut changed = false;

        for op in &tx_meta.operations.to_vec() {
            self.process_operation(op, &mut changed)?;
        }

        Ok(changed)
    }

    pub(crate) fn replace_binaries(
        &mut self,
        mercury_contracts: HashMap<Hash, &[u8]>,
    ) -> Result<bool, RetroshadeError> {
        let mut replaced = false;

        let binaries_mutation = {
            let mut binaries_mutation = HashMap::new();

            for entry in self.target_pre_execution_state.iter() {
                match &entry.0.data {
                    LedgerEntryData::ContractData(data) => {
                        let contract_hash = match &data.contract {
                            ScAddress::Contract(hash) => hash,
                            _ => return Err(RetroshadeError::MalformedXdr),
                        };
                        if let Some(new_code) = mercury_contracts.get(&contract_hash) {
                            if let ScVal::LedgerKeyContractInstance = data.key {
                                if let ScVal::ContractInstance(instance) = &data.val {
                                    if let ContractExecutable::Wasm(wasm) = &instance.executable {
                                        binaries_mutation.insert(wasm.clone(), new_code);
                                    }
                                };
                            }
                        }
                    }

                    _ => (),
                }
            }

            binaries_mutation
        };

        for entry in self.target_pre_execution_state.iter_mut() {
            if let LedgerEntryData::ContractCode(code_entry) = &mut entry.0.data {
                if let Some(new_code) = binaries_mutation.get(&code_entry.hash) {
                    replaced = true;
                    code_entry.code = new_code.to_vec().try_into().unwrap();
                }
            }
        }

        Ok(replaced)
    }

    fn process_operation(
        &mut self,
        op: &OperationMeta,
        changed: &mut bool,
    ) -> Result<(), RetroshadeError> {
        let mut current_state = None;

        for change in &op.changes.0.to_vec() {
            match change {
                LedgerEntryChange::State(state) => current_state = Some(state),
                LedgerEntryChange::Updated(_) => {
                    if let Some(pre_execution) = &current_state {
                        self.update_entries(pre_execution, changed);
                    }
                    current_state = None;
                }
                LedgerEntryChange::Created(entry) => {
                    self.remove_entry(entry, changed);
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn remove_entry(&mut self, current_state_entry: &LedgerEntry, changed: &mut bool) {
        // note: should only be one entry but we do this for consistency.
        let mut to_delete = Vec::new();

        let len_here = self.target_pre_execution_state.len();

        for (idx, entry) in self.target_pre_execution_state.iter().enumerate() {
            match &entry.0.data {
                LedgerEntryData::ContractCode(_) => {
                    // this should not happen in general. We ignore for wasm uploads.
                }
                LedgerEntryData::ContractData(data) => {
                    if let LedgerEntryData::ContractData(pre_data) = &current_state_entry.data {
                        if data.contract == pre_data.contract && data.key == pre_data.key {
                            to_delete.push(idx);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut shift = 0;
        for idx in to_delete {
            let target_idx_adjusted = idx - shift;

            if self.target_pre_execution_state.len() > target_idx_adjusted {
                self.target_pre_execution_state.remove(target_idx_adjusted);
                *changed = true;
                shift += 1;
            } else {
                log::error!("Unknown retorshades state error: Previous len before processing {}, idx: {}, current len: {}", len_here, idx, self.target_pre_execution_state.len())
            }
        }
    }

    fn update_entries(&mut self, pre_execution: &LedgerEntry, changed: &mut bool) {
        for entry in self.target_pre_execution_state.iter_mut() {
            match &entry.0.data {
                LedgerEntryData::ContractCode(code) => {
                    if let LedgerEntryData::ContractCode(pre_code) = &pre_execution.data {
                        if pre_code.hash == code.hash {
                            *entry = (pre_execution.clone(), entry.1);
                            *changed = true;
                        }
                    }
                }
                LedgerEntryData::ContractData(data) => {
                    if let LedgerEntryData::ContractData(pre_data) = &pre_execution.data {
                        if data.contract == pre_data.contract && data.key == pre_data.key {
                            *entry = (pre_execution.clone(), entry.1);
                            *changed = true;
                        }
                    }
                }
                LedgerEntryData::Trustline(data) => {
                    if let LedgerEntryData::Trustline(pre_data) = &pre_execution.data {
                        if data.asset == pre_data.asset && data.account_id == pre_data.account_id {
                            *entry = (pre_execution.clone(), entry.1);
                            *changed = true;
                        }
                    }
                }

                LedgerEntryData::Account(data) => {
                    if let LedgerEntryData::Account(pre_data) = &pre_execution.data {
                        if data.account_id == pre_data.account_id {
                            *entry = (pre_execution.clone(), entry.1);
                            *changed = true;
                        }
                    }
                }

                _ => {}
            }
        }
    }
}
