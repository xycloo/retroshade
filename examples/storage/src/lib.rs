#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, IntoVal, Val};

#[link(wasm_import_module = "x")]
extern "C" {
    #[allow(improper_ctypes)]
    #[link_name = "9"]
    pub fn zephyr_emit(target: i64, event: i64) -> i64;
}

#[contracttype]
pub struct FirstRetroshade {
    test: Address,
    amount: i128,
}

#[contract]
pub struct HelloContract;

#[contractimpl]
impl HelloContract {
    pub fn put(env: Env) {
        env.storage().instance().set(&0, &1_i128);
    }

    pub fn t(env: Env) -> () {
        if 1_i128 == env.storage().instance().get::<i32, i128>(&0).unwrap() {
            let target = symbol_short!("test").as_val().get_payload() as i64;
            let event = FirstRetroshade {
                test: env.current_contract_address(),
                amount: 2,
            };
            let event: Val = event.into_val(&env);
            let event = event.get_payload() as i64;

            unsafe { zephyr_emit(target, event) };

            env.storage().instance().set(&0, &2_i128);
        }
    }
}

mod test;
