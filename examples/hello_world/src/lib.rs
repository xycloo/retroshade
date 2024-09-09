#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, vec, Address, Env, IntoVal, String, Val,
    Vec,
};
/*
#[link(wasm_import_module = "x")]
extern "C" {
    #[allow(improper_ctypes)]
    #[link_name = "9"]
    pub fn zephyr_emit(target: i64, event: i64) -> i64;
}
*/
#[contracttype]
pub struct FirstRetroshade {
    test: Address,
}

#[contract]
pub struct HelloContract;

#[contractimpl]
impl HelloContract {
    pub fn t(env: Env) -> () {
        /*let target = symbol_short!("test").as_val().get_payload() as i64;
        let event = FirstRetroshade {
            test: env.current_contract_address(),
        };
        let event: Val = event.into_val(&env);
        let event = event.get_payload() as i64;

        unsafe { zephyr_emit(target, event) };*/
    }
}

mod test;
