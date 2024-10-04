//! This module handles the pretty-print of ScVals in order for them to be
//! consumed and potentially efficiently filtered within the db.

use std::error::Error;

use bytes::BytesMut;
use num_bigint::BigInt;
use num_traits::FromPrimitive;
use postgres_types::{to_sql_checked, IsNull, ToSql, Type};
use soroban_env_host::xdr::{
    Int128Parts, Int256Parts, PublicKey, ScAddress, ScVal, ScVec, UInt128Parts, UInt256Parts,
};

const MAX_ALLOWED_RECURSION_DEPTH: usize = 1;

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
    GenericArray(Vec<FromScVal>), // Note: max allowed recursion depth is one.
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

impl FromScVal {
    pub fn from_scval(value: ScVal, recursion_depth: &mut usize) -> Self {
        match value {
            ScVal::Bool(b) => FromScVal {
                dbtype: Type::BOOL,
                kind: TypeKind::Boolean(b),
            },
            ScVal::Void => FromScVal {
                dbtype: Type::TEXT,
                kind: TypeKind::Void,
            },
            ScVal::U32(n) => FromScVal {
                dbtype: Type::NUMERIC,
                kind: TypeKind::Numeric(n.to_string()),
            },
            ScVal::I32(n) => FromScVal {
                dbtype: Type::NUMERIC,
                kind: TypeKind::Numeric(n.to_string()),
            },
            ScVal::U64(n) => FromScVal {
                dbtype: Type::NUMERIC,
                kind: TypeKind::Numeric(n.to_string()),
            },
            ScVal::I64(n) => FromScVal {
                dbtype: Type::NUMERIC,
                kind: TypeKind::Numeric(n.to_string()),
            },
            ScVal::Timepoint(t) => FromScVal {
                dbtype: Type::NUMERIC,
                kind: TypeKind::Numeric(t.0.to_string()),
            },
            ScVal::Duration(d) => FromScVal {
                dbtype: Type::NUMERIC,
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
            ScVal::Vec(v) => {
                *recursion_depth += 1;

                if *recursion_depth <= MAX_ALLOWED_RECURSION_DEPTH {
                    if let Some(ScVec(vecm)) = &v {
                        let inner_array: Vec<FromScVal> = vecm
                            .iter()
                            .map(|element| FromScVal::from_scval(element.clone(), recursion_depth))
                            .collect();

                        if !inner_array.is_empty()
                            && inner_array
                                .iter()
                                .all(|item| item.dbtype == inner_array[0].dbtype)
                        {
                            let dbtype = match inner_array[0].kind {
                                TypeKind::Boolean(_) => Type::BOOL_ARRAY,
                                TypeKind::Numeric(_) => Type::NUMERIC_ARRAY,
                                TypeKind::Text(_) => Type::TEXT_ARRAY,
                                _ => Type::JSON,
                            };

                            if dbtype != Type::JSON {
                                return FromScVal {
                                    dbtype,
                                    kind: TypeKind::GenericArray(inner_array),
                                };
                            }
                        }
                    }
                }

                FromScVal {
                    dbtype: Type::JSON,
                    kind: TypeKind::Text(serde_json::to_string(&v).unwrap()),
                }
            }
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

impl ToSql for FromScVal {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        match &self.kind {
            TypeKind::GenericArray(arr) => {
                // For arrays, we need to handle each type separately
                match self.dbtype {
                    Type::BOOL_ARRAY => {
                        let bool_array: Vec<bool> = arr
                            .iter()
                            .filter_map(|item| match &item.kind {
                                TypeKind::Boolean(b) => Some(*b),
                                _ => None,
                            })
                            .collect();
                        bool_array.to_sql(ty, out)
                    }
                    Type::NUMERIC_ARRAY => {
                        let num_array: Vec<f64> = arr
                            .iter()
                            .filter_map(|item| match &item.kind {
                                TypeKind::Numeric(n) => Some(n.clone().parse().unwrap_or(0.0)),
                                _ => None,
                            })
                            .collect();
                        num_array.to_sql(ty, out)
                    }
                    Type::TEXT_ARRAY => {
                        let text_array: Vec<String> = arr
                            .iter()
                            .filter_map(|item| match &item.kind {
                                TypeKind::Text(s) => Some(s.clone()),
                                _ => None,
                            })
                            .collect();
                        text_array.to_sql(ty, out)
                    }
                    _ => Err("Unsupported array type".into()),
                }
            }
            TypeKind::Text(s) => s.to_sql(ty, out),
            TypeKind::Boolean(b) => b.to_sql(ty, out),
            TypeKind::Void => Ok(IsNull::Yes),
            TypeKind::Numeric(n) => {
                let n: f64 = n.parse().unwrap_or(0.0);
                n.to_sql(ty, out)
            }
        }
    }

    fn accepts(ty: &Type) -> bool {
        matches!(
            ty,
            &Type::BOOL
                | &Type::TEXT
                | &Type::FLOAT8
                | &Type::BYTEA
                | &Type::BOOL_ARRAY
                | &Type::TEXT_ARRAY
                | &Type::FLOAT8_ARRAY
                | &Type::JSONB
        )
    }

    to_sql_checked!();
}
