#[macro_export]
macro_rules! impl_diesel_sql_type {
    ($vis:vis struct $name:ident ($vis_inenr:vis $inner:ty) $(;)?) => {
        $crate::impl_diesel_sql_type!(@gen $name, $inner, 0);
    };

    ($vis:vis struct $name:ident { $vis_inner:vis $field:ident: $field_type:ty  $(,)? }) => {
        $crate::impl_diesel_sql_type!(@gen $name, $inner, $field_name);
    };

    (@gen $name:path, $inner:ty, $field_name:tt) => {
        const _: () = {
            impl<ST, DB> diesel::serialize::ToSql<ST, DB> for $name
            where
                $inner: diesel::serialize::ToSql<ST, DB>,
                DB: diesel::backend::Backend,
                DB: diesel::sql_types::HasSqlType<ST>,
            {
                fn to_sql<'b>(
                    &'b self,
                    out: &mut diesel::serialize::Output<'b, '_, DB>,
                ) -> diesel::serialize::Result {
                    self.$field_name.to_sql(out)
                }
            }

            impl<ST> diesel::expression::AsExpression<ST> for $name
            where
                diesel::internal::derives::as_expression::Bound<ST, $inner>:
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
                diesel::internal::derives::as_expression::Bound<ST, $inner>:
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
                diesel::internal::derives::as_expression::Bound<ST, $inner>:
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
                $inner: diesel::deserialize::FromSql<ST, DB>,
                DB: diesel::backend::Backend,
                DB: diesel::sql_types::HasSqlType<ST>,
            {
                fn from_sql(
                    raw: DB::RawValue<'_>,
                ) -> ::std::result::Result<Self, Box<dyn ::std::error::Error + Send + Sync>>
                {
                    diesel::deserialize::FromSql::<ST, DB>::from_sql(raw).map($name)
                }
            }
            impl<ST, DB> diesel::deserialize::Queryable<ST, DB> for $name
            where
                $inner: diesel::deserialize::FromStaticSqlRow<ST, DB>,
                DB: diesel::backend::Backend,
                DB: diesel::sql_types::HasSqlType<ST>,
            {
                type Row = $inner;
                fn build(row: Self::Row) -> diesel::deserialize::Result<Self> {
                    Ok($name(row))
                }
            }
            impl diesel::query_builder::QueryId for $name {
                type QueryId = Self;
            }
        };
    };
}
