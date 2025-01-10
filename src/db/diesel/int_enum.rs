#[macro_export]
#[doc(hidden)]
macro_rules! impl_enum_to_sql {
    (@gen $name:ident, $int_type:ty, $($variant:ident = $value:expr),*) => {
        const _: () = {
            impl<ST, DB> diesel::serialize::ToSql<ST, DB> for $name
            where
                $int_type: diesel::serialize::ToSql<ST, DB>,
                DB: diesel::backend::Backend,
                DB: diesel::sql_types::HasSqlType<ST>,
                for<'c> DB: diesel::backend::Backend<
                    BindCollector<'c> = diesel::query_builder::bind_collector::RawBytesBindCollector<DB>,
                >,
            {
                fn to_sql<'b>(
                    &'b self,
                    out: &mut diesel::serialize::Output<'b, '_, DB>,
                ) -> diesel::serialize::Result {
                    let value = match self {
                        $(Self::$variant => $value,)*
                    };

                    value.to_sql(&mut out.reborrow())?;

                    Ok(diesel::serialize::IsNull::No)
                }
            }

            impl<ST> diesel::expression::AsExpression<ST> for $name
            where
                diesel::internal::derives::as_expression::Bound<ST, $int_type>:
                    diesel::expression::Expression<SqlType = ST>,
                ST: diesel::sql_types::SingleValue,
            {
                type Expression = diesel::internal::derives::as_expression::Bound<ST, Self>;
                fn as_expression(self) -> Self::Expression {
                    diesel::internal::derives::as_expression::Bound::new(self)
                }
            }
            impl<'expr, ST> diesel::expression::AsExpression<ST> for &'expr $name
            where
                diesel::internal::derives::as_expression::Bound<ST, $int_type>:
                    diesel::expression::Expression<SqlType = ST>,
                ST: diesel::sql_types::SingleValue,
            {
                type Expression = diesel::internal::derives::as_expression::Bound<ST, Self>;
                fn as_expression(self) -> Self::Expression {
                    diesel::internal::derives::as_expression::Bound::new(self)
                }
            }
            impl<'expr2, 'expr, ST> diesel::expression::AsExpression<ST> for &'expr2 &'expr $name
            where
                diesel::internal::derives::as_expression::Bound<ST, $int_type>:
                    diesel::expression::Expression<SqlType = ST>,
                ST: diesel::sql_types::SingleValue,
            {
                type Expression = diesel::internal::derives::as_expression::Bound<ST, Self>;
                fn as_expression(self) -> Self::Expression {
                    diesel::internal::derives::as_expression::Bound::new(self)
                }
            }
            impl<ST, DB> diesel::deserialize::FromSql<ST, DB> for $name
            where
                $int_type: diesel::deserialize::FromSql<ST, DB>,
                DB: diesel::backend::Backend,
                DB: diesel::sql_types::HasSqlType<ST>,
            {
                fn from_sql(
                    raw: DB::RawValue<'_>,
                ) -> ::std::result::Result<Self, Box<dyn ::std::error::Error + Send + Sync>>
                {
                    let value: $int_type = diesel::deserialize::FromSql::<ST, DB>::from_sql(raw)?;
                    match value {
                        $($value => Ok(Self::$variant),)*
                        _ => {
                            let err = format!("invalid UserRole value from DB: {}", value);
                            Err(From::from(err))
                        }
                    }
                }
            }
            impl<ST, DB> diesel::deserialize::Queryable<ST, DB> for $name
            where
                $int_type: diesel::deserialize::FromStaticSqlRow<ST, DB>,
                DB: diesel::backend::Backend,
                DB: diesel::sql_types::HasSqlType<ST>,
            {
                type Row = $int_type;
                fn build(row: Self::Row) -> diesel::deserialize::Result<Self> {
                    match row {
                        $($value => Ok(Self::$variant),)*
                        _ => {
                            let err = format!("invalid UserRole value from DB: {}", row);
                            Err(From::from(err))
                        }
                    }
                }
            }
            impl diesel::query_builder::QueryId for $name {
                type QueryId = Self;
            }
        };
    };
}

/// This macro is used to make an enum serializable and deserializable to a smallint in Diesel.
/// The enum must be defined in other place, and then this macro is used to make it serializable and deserializable as i16.
/// So the enum type must be imported into the module where this macro is used.
///
/// # Example
/// ```rust
/// diesel_smallint_enum! {
///     pub enum UserRole {
///         Admin = 1,
///         User = 2,
///     }
/// }
/// ```
#[macro_export]
macro_rules! diesel_smallint_enum {
    ($vis:vis enum $name:ident { $($variant:ident = $value:expr),* $(,)? }) => {
        $crate::impl_enum_to_sql!(@gen $name, i16, $($variant = $value),*);
    };

}
