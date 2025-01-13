use crate::{db::TxnManager, provider::Provider, result::BizResult};

pub trait UseCase {
    type Input;
    type Output;
    type Error;

    async fn execute(&mut self, input: Self::Input) -> BizResult<Self::Output, Self::Error>;

    async fn execute_in_txn<Txn>(
        &mut self,
        mut txn: Txn,
        input: Self::Input,
    ) -> BizResult<Self::Output, Self::Error>
    where
        Txn: TxnManager,
    {
        txn.do_transaction(self.execute(input)).await
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
    Tx: TxnManager,
    UC: UseCase,
{
    type Input = UC::Input;

    type Output = UC::Output;

    type Error = UC::Error;

    async fn execute(&mut self, params: Self::Input) -> BizResult<Self::Output, Self::Error> {
        self.uc.execute_in_txn(self.tx.clone(), params).await
    }

    async fn execute_in_txn<Txn>(
        &mut self,
        txn: Txn,
        params: Self::Input,
    ) -> BizResult<Self::Output, Self::Error>
    where
        Txn: TxnManager,
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
            params: Self::Input,
        ) -> bagua::result::BizResult<Self::Output, Self::Error>
        where
            Txn: bagua::db::TxnManager,
        {
            // There's no need to use transaction in this UseCase
            self.execute(params).await
        }
    };
}
