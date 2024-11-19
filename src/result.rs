pub type BizResult<T, E> = anyhow::Result<Result<T, E>>;

pub trait MapBizResult<T, E>: Sized {
    fn map_biz_err<NE, F: FnOnce(E) -> NE>(self, f: F) -> BizResult<T, NE>;

    fn map_biz<NT, F: FnOnce(T) -> NT>(self, f: F) -> BizResult<NT, E>;
}

impl<T, E> MapBizResult<T, E> for BizResult<T, E> {
    fn map_biz_err<NE, F: FnOnce(E) -> NE>(self, f: F) -> BizResult<T, NE> {
        match self {
            Ok(Err(e)) => Ok(Err(f(e))),
            Ok(Ok(t)) => Ok(Ok(t)),
            Err(e) => Err(e),
        }
    }

    fn map_biz<NT, F: FnOnce(T) -> NT>(self, f: F) -> BizResult<NT, E> {
        match self {
            Ok(Ok(t)) => Ok(Ok(f(t))),
            Ok(Err(e)) => Ok(Err(e)),
            Err(e) => Err(e),
        }
    }
}

#[macro_export]
macro_rules! ensure_exist {
    ($predict:expr, $err:expr) => {
        match $predict {
            Some(v) => v,
            None => return Ok(Err($err.into())),
        }
    };
}

#[macro_export]
macro_rules! ensure_biz {
    ($predict:expr, $err:expr) => {
        if !$predict {
            return Ok(Err($err.into()));
        }
    };

    (not $predict:expr, $err:expr) => {
        if $predict {
            return Ok(Err($err.into()));
        }
    };

    ($call:expr) => {
        match $call {
            Ok(value) => value,
            Err(err) => return Ok(Err(err.into())),
        }
    };
}

#[macro_export]
macro_rules! biz_ok {
    ($data:expr) => {
        Ok(Ok($data))
    };
}

#[macro_export]
macro_rules! biz_err {
    ($err:expr) => {
        Ok(Err($err.into()))
    };
}
