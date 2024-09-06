//! This module handles the pretty-print of ScVals in order for them to be
//! consumed and potentially efficiently filtered within the db.

use num_bigint::BigInt;
use num_traits::FromPrimitive;
use postgres_types::Type;
use soroban_env_host::xdr::{
    Int128Parts, Int256Parts, PublicKey, ScAddress, ScVal, UInt128Parts, UInt256Parts,
};

pub fn i256_to_bigint(parts: Int256Parts) -> BigInt {
    let hi =
        (BigInt::from_i64(parts.hi_hi).unwrap() << 64) | BigInt::from_u64(parts.hi_lo).unwrap();
    let lo =
        (BigInt::from_u64(parts.lo_hi).unwrap() << 64) | BigInt::from_u64(parts.lo_lo).unwrap();
    (hi << 128) | lo
}

pub fn u256_to_bigint(parts: UInt256Parts) -> BigInt {
    let hi =
        (BigInt::from_u64(parts.hi_hi).unwrap() << 64) | BigInt::from_u64(parts.hi_lo).unwrap();
    let lo =
        (BigInt::from_u64(parts.lo_hi).unwrap() << 64) | BigInt::from_u64(parts.lo_lo).unwrap();
    (hi << 128) | lo
}

pub fn i128_to_bigint(parts: Int128Parts) -> BigInt {
    (BigInt::from_i64(parts.hi).unwrap() << 64) | BigInt::from_u64(parts.lo).unwrap()
}

pub fn u128_to_bigint(parts: UInt128Parts) -> BigInt {
    (BigInt::from_u64(parts.hi).unwrap() << 64) | BigInt::from_u64(parts.lo).unwrap()
}

pub fn num_to_string(parts: ScVal) -> String {
    match parts {
        ScVal::I256(parts) => i256_to_bigint(parts).to_string(),
        ScVal::U256(parts) => u256_to_bigint(parts).to_string(),
        ScVal::I128(parts) => i128_to_bigint(parts).to_string(),
        ScVal::U128(parts) => u128_to_bigint(parts).to_string(),
        _ => panic!(), // todo handle error
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeKind {
    TextArray(Vec<String>), // currently unused.
    Text(String),
    Boolean(bool),
    Void,
    Numeric(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FromScVal {
    pub dbtype: Type,
    pub kind: TypeKind,
}

impl From<ScVal> for FromScVal {
    fn from(value: ScVal) -> Self {
        match value {
            ScVal::Bool(b) => FromScVal {
                dbtype: Type::BOOL,
                kind: TypeKind::Boolean(b),
            },
            ScVal::Void => FromScVal {
                dbtype: Type::VOID,
                kind: TypeKind::Void,
            },
            ScVal::U32(n) => FromScVal {
                dbtype: Type::INT4,
                kind: TypeKind::Numeric(n.to_string()),
            },
            ScVal::I32(n) => FromScVal {
                dbtype: Type::INT4,
                kind: TypeKind::Numeric(n.to_string()),
            },
            ScVal::U64(n) => FromScVal {
                dbtype: Type::INT8,
                kind: TypeKind::Numeric(n.to_string()),
            },
            ScVal::I64(n) => FromScVal {
                dbtype: Type::INT8,
                kind: TypeKind::Numeric(n.to_string()),
            },
            ScVal::Timepoint(t) => FromScVal {
                dbtype: Type::TIMESTAMP,
                kind: TypeKind::Numeric(t.0.to_string()),
            },
            ScVal::Duration(d) => FromScVal {
                dbtype: Type::INTERVAL,
                kind: TypeKind::Numeric(d.0.to_string()),
            },
            ScVal::U256(_) => FromScVal {
                dbtype: Type::NUMERIC,
                kind: TypeKind::Numeric(num_to_string(value)),
            },
            ScVal::I256(_) => FromScVal {
                dbtype: Type::NUMERIC,
                kind: TypeKind::Numeric(num_to_string(value)),
            },
            ScVal::Bytes(b) => FromScVal {
                dbtype: Type::BYTEA,
                kind: TypeKind::Text(hex::encode(b)),
            },
            ScVal::String(s) => FromScVal {
                dbtype: Type::TEXT,
                kind: TypeKind::Text(s.to_string()),
            },
            ScVal::Symbol(s) => FromScVal {
                dbtype: Type::TEXT,
                kind: TypeKind::Text(s.to_string()),
            },
            ScVal::Vec(v) => FromScVal {
                dbtype: Type::JSON,
                kind: TypeKind::Text(serde_json::to_string(&v).unwrap()),
            },
            ScVal::Map(m) => FromScVal {
                dbtype: Type::JSON,
                kind: TypeKind::Text(serde_json::to_string(&m).unwrap()),
            },
            ScVal::Error(e) => FromScVal {
                dbtype: Type::TEXT,
                kind: TypeKind::Text(serde_json::to_string(&e).unwrap()),
            },
            ScVal::Address(addr) => {
                let address = match addr {
                    ScAddress::Account(id) => {
                        let PublicKey::PublicKeyTypeEd25519(int) = id.0;
                        stellar_strkey::ed25519::PublicKey(int.0).to_string()
                    }
                    ScAddress::Contract(id) => stellar_strkey::Contract(id.0).to_string(),
                };

                FromScVal {
                    dbtype: Type::TEXT,
                    kind: TypeKind::Text(address),
                }
            }
            ScVal::I128(_) => FromScVal {
                dbtype: Type::NUMERIC,
                kind: TypeKind::Numeric(num_to_string(value)),
            },
            ScVal::U128(_) => FromScVal {
                dbtype: Type::NUMERIC,
                kind: TypeKind::Numeric(num_to_string(value)),
            },

            // this should not be reachable in a sane execution.
            _ => FromScVal {
                dbtype: Type::TEXT,
                kind: TypeKind::Text("Invalid".to_string()),
            },
        }
    }
}
