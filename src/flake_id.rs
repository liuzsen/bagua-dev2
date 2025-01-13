#[macro_export]
macro_rules! flake_id {
    ($type_name:ident $($tt:tt)*) => {
        $crate::flake_id!(@id_func $type_name $($tt)* ,);
        impl Default for $type_name {
            fn default() -> Self {
                Self::generate()
            }
        }

        impl<'a> From<&'a $type_name> for i64 {
            fn from(id: &'a $type_name) -> Self {
                id.0
            }
        }

        impl<'a> From<&'a $type_name> for $type_name {
            fn from(id: &'a $type_name) -> Self {
                *id
            }
        }

        impl $crate::entity::SysId for $type_name {
            fn generate() -> Self {
                Self::generate()
            }
        }

        impl $type_name {
            pub fn from_str(s: &str) -> Result<Self, std::num::ParseIntError> {
                Ok(Self(s.parse()?))
            }
        }
    };

    (@id_func $type_name:ident, node = $node:expr, $($tt:tt)*) => {
        impl $type_name {
            pub fn generate() -> $type_name {
                use $crate::flaken::Flaken;
                use ::std::sync::{Mutex, OnceLock};
                static FLAKE_ID_GENERATOR: OnceLock<Mutex<Flaken>> = OnceLock::new();
                let f = FLAKE_ID_GENERATOR.get_or_init(|| {
                    let f = $crate::flaken::Flaken::default();
                    let f = f.node($node as u64);
                    Mutex::new(f)
                });
                let mut lock = f.lock().unwrap();
                $type_name(lock.next() as i64)
            }
        }
        $crate::flake_id!(@impl $type_name, {Debug,PartialEq,PartialOrd,Eq,Hash,Clone,Copy,}, {}, $($tt)* ,);
    };

    (@id_func $type_name:ident, $($tt:tt)*) => {
        impl $type_name {
            pub fn generate() -> $type_name {
                use $crate::flaken::Flaken;
                use ::std::sync::{Mutex, OnceLock};
                static FLAKE_ID_GENERATOR: OnceLock<Mutex<Flaken>> = OnceLock::new();
                let f = FLAKE_ID_GENERATOR.get_or_init(|| {
                    let f = $crate::flaken::Flaken::default();
                    Mutex::new(f)
                });
                let mut lock = f.lock().unwrap();
                $type_name(lock.next() as i64)
            }
        }


        $crate::flake_id!(@impl $type_name, {Debug,PartialEq,PartialOrd,Eq,Hash,Clone,Copy,}, {}, $($tt)* ,);
    };


    (@impl $type_name:ident, {$($derives:tt)*}, {$($attrs:tt)*}, @redis, $($tail:tt)*) => {
        impl ::redis::ToRedisArgs for $type_name {
            fn write_redis_args<W: ?Sized>(&self, out: &mut W)
            where
                W: ::redis::RedisWrite,
            {
                self.0.write_redis_args(out)
            }
        }

        impl ::redis::FromRedisValue for $type_name {
            fn from_redis_value(v: &redis::Value) -> ::redis::RedisResult<Self> {
                let id = i64::from_redis_value(v)?;
                Ok($type_name(id))
            }
        }

        $crate::flake_id!(@impl $type_name, {$($derives)*}, {$($attrs)*}, $($tail)* ,);
    };

    (@impl $type_name:ident, {$($derives:tt)*}, {$($attrs:tt)*}, @serde, $($tail:tt)*) => {
        impl ::serde::Serialize for $type_name {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                serializer.serialize_str(&self.0.to_string())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $type_name {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                let id = String::deserialize(deserializer)?;
                let id = id.parse().map_err(serde::de::Error::custom)?;
                Ok(Self(id))
            }
        }

        $crate::flake_id!(@impl $type_name, {$($derives)*}, {$($attrs)*}, $($tail)* ,);
    };

    (@impl $type_name:ident, {$($derives:tt)*}, {$($attrs:tt)*}, @diesel-pg, $($tail:tt)*) => {
        const _: () = {
            use ::diesel::{
                backend::Backend,
                deserialize::{self, FromSql},
                sql_types::BigInt,
                serialize::{self, Output, ToSql},
            };
            type DbType = ::diesel::pg::Pg;

            impl ToSql<BigInt, DbType> for $type_name {
                fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DbType>) -> serialize::Result {
                    ToSql::<BigInt, DbType>::to_sql(&self.0, out)
                }
            }

            impl FromSql<BigInt, DbType> for $type_name {
                fn from_sql(bytes: <DbType as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
                    let res = FromSql::<BigInt, DbType>::from_sql(bytes)?;
                    Ok(Self(res))
                }
            }
        };
        $crate::flake_id!(@impl $type_name,
            {$($derives)* ::diesel:: AsExpression, ::diesel::FromSqlRow,},
            {$($attrs)* #[diesel(sql_type = ::diesel::sql_types::BigInt)] },
             $($tail)* ,
        );
    };

    (@impl $type_name:ident, {$($derives:tt)*}, {$($attrs:tt)*}, @graphql, $($tail:tt)*) => {
        ::async_graphql::scalar!($type_name);
        $crate::flake_id!(@impl $type_name, {$($derives)*}, {$($attrs)*}, $($tail)* ,);
    };

    (@impl $type_name:ident, {$($derives:tt)*}, {$($attrs:tt)*}, @!debug, $($tail:tt)*) => {
        $crate::un_derive!(@debug $type_name, {$($derives)*}, {$($attrs)*}, $($tail)* ,);
    };

    (@undebug $type_name:ident, {$($d_head:tt)* Debug, $(d_tail:tt)*}, {$($attrs:tt)*}, $($tail:tt)*) => {
        $crate::flake_id!(@impl $type_name, {$($d_head)* $($d_tail)*}, {$($attrs)*}, $($tail)* ,);
    };


    (@impl $type_name:ident, {$($derives:tt)*}, {$($attrs:tt)*}, $(,)*) => {
        use $crate::derive_more;

        #[derive(
        $($derives)*
        $crate::derive_more::From,
        $crate::derive_more::Display,
        $crate::derive_more::FromStr,
        )]
        $($attrs)*
        pub struct $type_name(pub i64);
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! un_derive {
    (@debug $type_name:ident, {Debug, $($derives:tt)*}, {$($attrs:tt)*}, $($tail:tt)*) => {
        $crate::flake_id!(@impl $type_name, {$($derives)*}, {$($attrs)*}, $($tail)* ,);
    };

    (@debug $type_name:ident, { $first:tt, $(d_tail:tt)*}, {$($attrs:tt)*}, $($tail:tt)*) => {
        $crate::un_derive!(@debug $type_name, {$(d_tail)* $first,}, {$($attrs)*}, $($tail)* ,);
    };
}

#[cfg(test)]
mod tests {
    flake_id!(UserId);

    #[test]
    fn t_flake_id() {
        let id1 = UserId::generate();
        let id2 = UserId::generate();
        assert_ne!(id1, id2);
    }
}
