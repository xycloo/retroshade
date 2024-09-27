#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, vec, Address, Env, IntoVal, String, Val,
    Vec,
};

#[cfg(feature = "mercury")]
#[link(wasm_import_module = "x")]
extern "C" {
    #[allow(improper_ctypes)]
    #[link_name = "9"]
    pub fn zephyr_emit(target: i64, event: i64) -> i64;
}

#[cfg(feature = "mercury")]
fn emit_deposit(env: &Env, from: Address, amount: i128, previous_tvl: i128) {
    let event = DepositEvent {
        from,
        amount,
        previous_tvl,
        now_tvl: previous_tvl + amount,
        ledger: env.ledger().sequence(),
        timestamp: env.ledger().timestamp(),
    };

    let target = symbol_short!("mydeposit").as_val().get_payload() as i64;
    let event: Val = event.into_val(env);
    let event = event.get_payload() as i64;

    unsafe { zephyr_emit(target, event) };
}

#[cfg(feature = "mercury")]
#[contracttype]
pub struct DepositEvent {
    previous_tvl: i128,
    now_tvl: i128,
    from: Address,
    amount: i128,
    ledger: u32,
    timestamp: u64,
}

#[contract]
pub struct HelloContract;

#[contractimpl]
impl HelloContract {
    pub fn deposit(env: Env, from: Address, amount: i128) {
        let current_tvl = env.storage().instance().get(&0).unwrap_or(0_i128);
        env.storage().instance().set(&0, &(current_tvl + amount));

        #[cfg(feature = "mercury")]
        emit_deposit(&env, from, amount, current_tvl)
    }
}

mod test;
