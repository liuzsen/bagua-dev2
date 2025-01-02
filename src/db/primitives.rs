use std::borrow::Cow;

use ::diesel::expression::AsExpression;
use serde::{Deserialize, Serialize};

use crate::entity::field::Field;

pub trait DbPrimitive<'a, DP> {
    fn to_domain_primitive(self) -> anyhow::Result<DP>;

    fn from_domain_primitive(do_obj: &'a DP) -> Self;
}

macro_rules! impl_db_primitive {
    ($ty:ty) => {
        impl<'a> DbPrimitive<'a, $ty> for $ty {
            fn to_domain_primitive(self) -> anyhow::Result<$ty> {
                Ok(self)
            }

            fn from_domain_primitive(do_obj: &'a $ty) -> Self {
                do_obj.clone()
            }
        }

        impl<'a> DbPrimitive<'a, $ty> for &'a $ty {
            fn to_domain_primitive(self) -> anyhow::Result<$ty> {
                Ok(self.clone())
            }

            fn from_domain_primitive(do_obj: &'a $ty) -> Self {
                do_obj
            }
        }

        impl<'a> DbPrimitive<'a, $ty> for std::borrow::Cow<'a, $ty> {
            fn to_domain_primitive(self) -> anyhow::Result<$ty> {
                Ok(self.into_owned())
            }

            fn from_domain_primitive(do_obj: &'a $ty) -> Self {
                std::borrow::Cow::Borrowed(do_obj)
            }
        }
    };
}

impl_db_primitive!(String);
impl_db_primitive!(bool);
impl_db_primitive!(i8);
impl_db_primitive!(i16);
impl_db_primitive!(i32);
impl_db_primitive!(i64);
impl_db_primitive!(u8);
impl_db_primitive!(u16);
impl_db_primitive!(u32);
impl_db_primitive!(u64);
impl_db_primitive!(f32);
impl_db_primitive!(f64);

impl<'a> DbPrimitive<'a, String> for Cow<'a, str> {
    fn to_domain_primitive(self) -> anyhow::Result<String> {
        Ok(self.to_string())
    }

    fn from_domain_primitive(do_obj: &'a String) -> Self {
        std::borrow::Cow::Borrowed(do_obj)
    }
}

impl<'a> DbPrimitive<'a, String> for &'a str {
    fn to_domain_primitive(self) -> anyhow::Result<String> {
        Ok(self.to_string())
    }

    fn from_domain_primitive(do_obj: &'a String) -> Self {
        do_obj
    }
}

impl<T> Field<T> {
    pub fn to_db_primitive<'a, P>(&'a self) -> P
    where
        P: DbPrimitive<'a, T>,
    {
        let v_ref = self.value_ref();
        DbPrimitive::from_domain_primitive(v_ref)
    }

    pub fn to_db_primitive_update<'a, P>(&'a self) -> Option<P>
    where
        P: DbPrimitive<'a, T>,
    {
        match self {
            Field::Unloaded => None,
            Field::Unchanged(_) => None,
            Field::Set(v) => Some(DbPrimitive::from_domain_primitive(v)),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, AsExpression)]
#[diesel(sql_type = ::diesel::sql_types::Json)]
#[diesel(sql_type = ::diesel::sql_types::Jsonb)]
pub struct JsonProxy<T>(pub T);

impl<'a, T> DbPrimitive<'a, T> for JsonProxy<T>
where
    T: Clone,
{
    fn to_domain_primitive(self) -> anyhow::Result<T> {
        Ok(self.0)
    }

    fn from_domain_primitive(do_obj: &'a T) -> Self {
        JsonProxy(do_obj.clone())
    }
}

impl<'a, T> DbPrimitive<'a, T> for JsonProxy<&'a T>
where
    T: Clone,
{
    fn to_domain_primitive(self) -> anyhow::Result<T> {
        Ok(self.0.clone())
    }

    fn from_domain_primitive(do_obj: &'a T) -> Self {
        JsonProxy(do_obj)
    }
}

impl<'a, T> DbPrimitive<'a, T> for JsonProxy<Cow<'a, T>>
where
    T: Clone,
{
    fn to_domain_primitive(self) -> anyhow::Result<T> {
        Ok(self.0.into_owned())
    }

    fn from_domain_primitive(do_obj: &'a T) -> Self {
        JsonProxy(std::borrow::Cow::Borrowed(do_obj))
    }
}

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
