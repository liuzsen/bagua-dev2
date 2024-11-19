use crate::result::BizResult;

pub trait UseCase {
    type Params;
    type Output;
    type Error;

    async fn execute(&mut self, params: Self::Params) -> BizResult<Self::Output, Self::Error>;
}
