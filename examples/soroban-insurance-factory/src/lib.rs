#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, vec, Address, BytesN, Env,
    IntoVal, String, Symbol, Val, Vec,
};

#[cfg(feature = "mercury")]
use retroshade_sdk::Retroshade;

#[contracttype]
pub enum DataKey {
    PoolHash,
}

#[derive(Copy, Clone, Debug)]
#[contracterror]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 0,
    NotInitialized = 1,
    NotAdmin = 2,
    PoolExists = 3,
    NoPool = 4,
}

#[cfg(feature = "mercury")]
#[derive(Retroshade)]
#[contracttype]
pub struct LiquidityPools {
    pools: Address,
}

pub fn set_pool_hash(env: &Env, pool_hash: BytesN<32>) {
    let key = DataKey::PoolHash;
    env.storage().instance().set(&key, &pool_hash);
}

pub fn get_pool_hash(env: &Env) -> BytesN<32> {
    let key = DataKey::PoolHash;
    env.storage().instance().get(&key).unwrap()
}

pub fn has_pool_hash(env: &Env) -> bool {
    let key = DataKey::PoolHash;
    env.storage().instance().has(&key)
}

#[contract]
pub struct HelloContract;

#[contractimpl]
impl HelloContract {
    pub fn init_factory(env: Env, pool_hash: BytesN<32>) -> Result<(), Error> {
        if has_pool_hash(&env) {
            return Err(Error::AlreadyInitialized);
        }

        set_pool_hash(&env, pool_hash);

        Ok(())
    }

    pub fn initialize(
        env: Env,
        admin: Address,
        salt: BytesN<32>,
        token: Address,
        oracle: Address,
        symbol: Symbol,
        external_asset: bool,
        oracle_asset: Option<Address>,
        periods_in_days: i32,
        volatility: i128,
        multiplier: i32,
    ) -> Result<Address, Error> {
        let pool_hash = get_pool_hash(&env);
        let pool_address = env.deployer().with_current_contract(salt).deploy(pool_hash);
        let res: Val = env.invoke_contract(
            &pool_address,
            &Symbol::new(&env, "initialize"),
            (
                admin.clone(),
                token,
                oracle,
                symbol,
                external_asset,
                oracle_asset,
                periods_in_days,
                volatility,
                multiplier,
            )
                .into_val(&env),
        );

        #[cfg(feature = "mercury")]
        LiquidityPools {
            pools: pool_address.clone(),
        }
        .emit(&env);

        Ok(pool_address)
    }
}

mod test;
