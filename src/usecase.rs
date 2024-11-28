use crate::{db::transaction::TransactionMaker, provider::Provider, result::BizResult};

pub trait UseCase {
    type Params;
    type Output;
    type Error;

    async fn execute(&mut self, params: Self::Params) -> BizResult<Self::Output, Self::Error>;

    async fn execute_in_txn<Txn>(
        &mut self,
        mut txn: Txn,
        params: Self::Params,
    ) -> BizResult<Self::Output, Self::Error>
    where
        Txn: TransactionMaker,
    {
        txn.do_transaction(self.execute(params)).await
    }
}

pub struct TxnUseCase<Tx, UC> {
    tx: Tx,
    uc: UC,
}

impl<Tx, UC> Provider for TxnUseCase<Tx, UC>
where
    Tx: Provider,
    UC: Provider,
{
    fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
        Ok(Self {
            tx: Tx::build(ctx)?,
            uc: UC::build(ctx)?,
        })
    }
}

impl<Tx, UC> UseCase for TxnUseCase<Tx, UC>
where
    Tx: TransactionMaker,
    UC: UseCase,
{
    type Params = UC::Params;

    type Output = UC::Output;

    type Error = UC::Error;

    async fn execute(&mut self, params: Self::Params) -> BizResult<Self::Output, Self::Error> {
        self.uc.execute_in_txn(self.tx.clone(), params).await
    }

    async fn execute_in_txn<Txn>(
        &mut self,
        txn: Txn,
        params: Self::Params,
    ) -> BizResult<Self::Output, Self::Error>
    where
        Txn: TransactionMaker,
    {
        self.uc.execute_in_txn(txn, params).await
    }
}

#[macro_export]
macro_rules! no_transaction {
    () => {
        async fn execute_in_txn<Txn>(
            &mut self,
            _txn: Txn,
            params: Self::Params,
        ) -> bagua::result::BizResult<Self::Output, Self::Error>
        where
            Txn: bagua::db::transaction::TransactionMaker,
        {
            // We don't need to use transaction in this UseCase
            self.execute(params).await
        }
    };
}
