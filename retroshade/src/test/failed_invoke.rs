use crate::{RetroshadeError, RetroshadeExecutionResult, RetroshadesExecution};
use soroban_env_host::{
    xdr::{Hash, ScMap, ScSymbol, ScVal},
    zephyr::RetroshadeExport,
    LedgerInfo,
};

#[test]
fn failed_invoke_fails_closed() {
    let execution = RetroshadesExecution::new(LedgerInfo::default());

    let exec_result = RetroshadeExecutionResult {
        retroshades: vec![RetroshadeExport {
            contract_id: Hash([0; 32]),
            target: ScVal::Symbol(ScSymbol("test".try_into().unwrap())),
            event_object: ScVal::Map(Some(ScMap(vec![].try_into().unwrap()))),
        }],
        diagnostic: vec![],
        invoke_success: false,
    };

    let result = execution.retroshade_prepare_for_db(exec_result);
    assert!(matches!(
        result,
        Err(RetroshadeError::NonSuccessfulContractCall(_))
    ));
}

#[test]
fn successful_invoke_passes() {
    let execution = RetroshadesExecution::new(LedgerInfo::default());

    let exec_result = RetroshadeExecutionResult {
        retroshades: vec![RetroshadeExport {
            contract_id: Hash([0; 32]),
            target: ScVal::Symbol(ScSymbol("test".try_into().unwrap())),
            event_object: ScVal::Map(Some(ScMap(vec![].try_into().unwrap()))),
        }],
        diagnostic: vec![],
        invoke_success: true,
    };

    let result = execution.retroshade_prepare_for_db(exec_result);
    assert!(result.is_ok());
}
