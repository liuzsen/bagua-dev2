use std::borrow::Cow;

use ::diesel::expression::AsExpression;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, AsExpression)]
#[diesel(sql_type = ::diesel::sql_types::Json)]
#[diesel(sql_type = ::diesel::sql_types::Jsonb)]
pub struct JsonProxy<T>(pub T);

#[cfg(feature = "diesel")]
mod diesel_impl {
    use std::fmt::Debug;

    use diesel::{
        backend::Backend,
        query_builder::bind_collector::RawBytesBindCollector,
        serialize::ToSql,
        sql_types::{Json, Jsonb},
    };
    use serde::Serialize;

    use super::JsonProxy;

    type DieselDeserializeError = Box<dyn std::error::Error + Send + Sync>;

    impl<DB, T> ToSql<Jsonb, DB> for JsonProxy<T>
    where
        DB: Backend,
        T: Debug,
        T: Serialize,
        serde_json::Value: ToSql<Jsonb, DB>,
        for<'c> DB: Backend<BindCollector<'c> = RawBytesBindCollector<DB>>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            let value = serde_json::to_value(&self).map_err(|err| {
                let e = format!("failed to serialize value: {}", err);
                Box::<dyn std::error::Error + Send + Sync>::from(e)
            })?;
            value.to_sql(&mut out.reborrow())
        }
    }

    impl<DB, T> ToSql<Json, DB> for JsonProxy<T>
    where
        DB: Backend,
        T: Debug,
        T: Serialize,
        serde_json::Value: ToSql<Json, DB>,
        for<'c> DB: Backend<BindCollector<'c> = RawBytesBindCollector<DB>>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            let value = serde_json::to_value(&self).map_err(|err| {
                let e = format!("failed to serialize value: {}", err);
                DieselDeserializeError::from(e)
            })?;
            value.to_sql(&mut out.reborrow())
        }
    }

    #[cfg(feature = "diesel-postgres")]
    mod postgres_impl {
        use diesel::{backend::Backend, deserialize::FromSql, pg::Pg, sql_types::Jsonb};
        use serde::de::DeserializeOwned;

        use crate::db::primitives::JsonProxy;

        use super::DieselDeserializeError;

        impl<T> FromSql<Jsonb, Pg> for JsonProxy<T>
        where
            T: std::fmt::Debug,
            T: DeserializeOwned,
        {
            fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
                let v: Self = serde_json::from_slice(bytes.as_bytes()).map_err(|err| {
                    let e = format!("corrupted db json value: {}", err);
                    DieselDeserializeError::from(e)
                })?;
                Ok(v)
            }
        }
    }

    #[cfg(feature = "diesel-mysql")]
    mod mysql_impl {
        use diesel::{deserialize::FromSql, mysql::Mysql, sql_types::Json};
        use serde::de::DeserializeOwned;

        use crate::db::primitives::JsonProxy;

        use super::DieselDeserializeError;

        impl<T> FromSql<Json, Mysql> for JsonProxy<T>
        where
            T: std::fmt::Debug,
            T: DeserializeOwned,
        {
            fn from_sql(
                value: <Mysql as diesel::backend::Backend>::RawValue<'_>,
            ) -> diesel::deserialize::Result<Self> {
                let v: Self = serde_json::from_slice(value.as_bytes()).map_err(|err| {
                    let e = format!("corrupted db json value: {}", err);
                    DieselDeserializeError::from(e)
                })?;
                Ok(v)
            }
        }
    }
}

pub trait PersistentObject<'a, DO> {
    fn from_domain_object(do_obj: &'a DO) -> Self;
}

pub trait ToDbPrimitive<'a, T> {
    fn to_db_primitive(&'a self) -> T;
}

pub trait ToDomainPremitive<T> {
    fn to_domain_primitive(self) -> T;
}

impl<T> ToDomainPremitive<T> for T {
    fn to_domain_primitive(self) -> T {
        self
    }
}

impl<'a, T> ToDbPrimitive<'a, &'a T> for T {
    fn to_db_primitive(&'a self) -> &'a T {
        self
    }
}

impl<'a, T> ToDbPrimitive<'a, Cow<'a, T>> for T
where
    T: ToOwned,
{
    fn to_db_primitive(&'a self) -> Cow<'a, T> {
        Cow::Borrowed(self)
    }
}

pub trait WrapperType: Sized {
    type InnerType;

    fn get_inner(&self) -> &Self::InnerType;

    fn from_inner(s: Self::InnerType) -> Result<Self, String>;
}
