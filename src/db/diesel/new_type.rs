#[macro_export]
macro_rules! diesel_sql_type_wrapper {
    ($vis:vis struct $name:ident ($vis_inenr:vis $inner:ty) $(;)?) => {
        $crate::diesel_sql_type_wrapper!(@gen $name, $inner);
    };

    ($vis:vis struct $name:ident { $vis_inner:vis $field:ident: $field_type:ty  $(,)? }) => {
        $crate::diesel_sql_type_wrapper!(@gen $name, $inner);
    };

    (@gen $name:path, $inner:ty) => {
        const _: () = {
            use bagua::db::primitives::WrapperType;

            const fn type_test<T: WrapperType<InnerType = $inner>>() {}

            type_test::<$name>();

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
                    self.get_inner().to_sql(out)
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
                    let inner = diesel::deserialize::FromSql::<ST, DB>::from_sql(raw)?;
                    Self::from_inner(inner).map_err(|e| From::from(e))
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
                    Self::from_inner(row).map_err(|e| From::from(e))
                }
            }
            impl diesel::query_builder::QueryId for $name {
                type QueryId = Self;
            }
        };
    };
}
