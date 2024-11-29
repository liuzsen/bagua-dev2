use std::{borrow::Cow, sync::Arc};

/// This trait will have some breaking changes after actix-web 5.0 is released
/// See: https://github.com/actix/actix-web/issues/3384
pub trait HttpRequest {
    fn method(&self) -> http::Method;

    fn version(&self) -> http::Version;

    fn uri(&self) -> http::Uri;

    fn path(&self) -> &str;

    fn headers(&self) -> Cow<http::HeaderMap>;
}

pub trait HttpJsonBody<T> {
    fn get_body(self) -> T;
}

pub trait HttpJsonQuery<T> {
    fn get_query(self) -> T;
}

pub trait HttpCredential {
    type Credential;

    fn credential(&self) -> Self::Credential;
}

pub struct IdCredential<T> {
    pub id: Arc<T>,
}

pub struct HttpRequestCompose<A, B> {
    this: A,
    that: B,
}

pub struct HttpCredentialCompose<A, B> {
    this: A,
    that: B,
}

pub struct HttpJsonBodyCompose<A, B> {
    this: A,
    that: B,
}

pub struct HttpJsonQueryCompose<A, B> {
    this: A,
    that: B,
}

pub struct ComposeNil;

impl<A, B> HttpRequest for HttpRequestCompose<A, B>
where
    A: HttpRequest,
{
    fn method(&self) -> http::Method {
        self.this.method()
    }

    fn version(&self) -> http::Version {
        self.this.version()
    }

    fn uri(&self) -> http::Uri {
        self.this.uri()
    }

    fn path(&self) -> &str {
        self.this.path()
    }

    fn headers(&self) -> std::borrow::Cow<'_, http::HeaderMap> {
        self.this.headers()
    }
}

impl<A, B, T> HttpJsonBody<T> for HttpJsonBodyCompose<A, B>
where
    A: HttpJsonBody<T>,
{
    fn get_body(self) -> T {
        self.this.get_body()
    }
}

impl<A, B, T> HttpJsonQuery<T> for HttpJsonQueryCompose<A, B>
where
    A: HttpJsonQuery<T>,
{
    fn get_query(self) -> T {
        self.this.get_query()
    }
}

impl<A, B> HttpCredential for HttpCredentialCompose<A, B>
where
    A: HttpCredential,
{
    type Credential = A::Credential;

    fn credential(&self) -> Self::Credential {
        self.this.credential()
    }
}

macro_rules! impl_http_request_for_that {
    ($compose:ident) => {
        impl<A, B> HttpRequest for $compose<A, B>
        where
            B: HttpRequest,
        {
            fn method(&self) -> http::Method {
                self.that.method()
            }

            fn version(&self) -> http::Version {
                self.that.version()
            }

            fn uri(&self) -> http::Uri {
                self.that.uri()
            }

            fn path(&self) -> &str {
                self.that.path()
            }

            fn headers(&self) -> std::borrow::Cow<http::HeaderMap> {
                self.that.headers()
            }
        }
    };
}

macro_rules! impl_http_json_body_for_that {
    ($compose:ident) => {
        impl<A, B, T> HttpJsonBody<T> for $compose<A, B>
        where
            B: HttpJsonBody<T>,
        {
            fn get_body(self) -> T {
                self.that.get_body()
            }
        }
    };
}

macro_rules! impl_http_json_query_for_that {
    ($compose:ident) => {
        impl<A, B, T> HttpJsonQuery<T> for $compose<A, B>
        where
            B: HttpJsonQuery<T>,
        {
            fn get_query(self) -> T {
                self.that.get_query()
            }
        }
    };
}

macro_rules! impl_credential_for_that {
    ($compose:ident) => {
        impl<A, B> HttpCredential for $compose<A, B>
        where
            B: HttpCredential,
        {
            type Credential = B::Credential;

            fn credential(&self) -> Self::Credential {
                self.that.credential()
            }
        }
    };
}

impl_http_json_body_for_that!(HttpRequestCompose);
impl_http_json_query_for_that!(HttpRequestCompose);
impl_credential_for_that!(HttpRequestCompose);

impl_http_request_for_that!(HttpJsonBodyCompose);
impl_http_json_query_for_that!(HttpJsonBodyCompose);
impl_credential_for_that!(HttpJsonBodyCompose);

impl_http_request_for_that!(HttpJsonQueryCompose);
impl_http_json_body_for_that!(HttpJsonQueryCompose);
impl_credential_for_that!(HttpJsonQueryCompose);

impl_http_request_for_that!(HttpCredentialCompose);
impl_http_json_body_for_that!(HttpCredentialCompose);
impl_http_json_query_for_that!(HttpCredentialCompose);

#[cfg(feature = "actix-web")]
pub mod actix_web_impl {
    use std::{borrow::Cow, convert::Infallible, str::FromStr, sync::Arc};

    use actix_web::FromRequest;
    use futures::{future::LocalBoxFuture, FutureExt};

    use super::{
        ComposeNil, HttpCredential, HttpCredentialCompose, HttpJsonBody, HttpJsonBodyCompose,
        HttpJsonQuery, HttpJsonQueryCompose, HttpRequest, HttpRequestCompose,
    };

    pub type HttpRequestImpl = actix_web::HttpRequest;
    pub type HttpJsonBodyImpl<T> = actix_web::web::Json<T>;
    pub type HttpJsonQueryImpl<T> = actix_web::web::Query<T>;
    pub type HttpCredentialImpl<T> = ActixIdentityCredential<T>;

    impl FromRequest for ComposeNil {
        type Error = Infallible;

        type Future = std::future::Ready<Result<Self, Self::Error>>;

        fn from_request(
            _req: &actix_web::HttpRequest,
            _payload: &mut actix_web::dev::Payload,
        ) -> Self::Future {
            std::future::ready(Ok(ComposeNil))
        }
    }

    macro_rules! try_result {
        ($expr:expr) => {
            match $expr {
                Ok(val) => val,
                Err(err) => return Err(err.into()),
            }
        };
    }

    macro_rules! compose_impl_from_request {
        ($compose:ident) => {
            impl<A, B> FromRequest for $compose<A, B>
            where
                A: FromRequest,
                B: FromRequest,
            {
                type Error = actix_web::Error;

                type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

                fn from_request(
                    req: &actix_web::HttpRequest,
                    payload: &mut actix_web::dev::Payload,
                ) -> Self::Future {
                    let req = req.clone();
                    let mut payload = payload.take();

                    async move {
                        let a = try_result!(A::from_request(&req, &mut payload).await);
                        let b = try_result!(B::from_request(&req, &mut payload).await);

                        Ok($compose { this: a, that: b })
                    }
                    .boxed_local()
                }
            }
        };
    }

    compose_impl_from_request!(HttpRequestCompose);
    compose_impl_from_request!(HttpCredentialCompose);
    compose_impl_from_request!(HttpJsonBodyCompose);
    compose_impl_from_request!(HttpJsonQueryCompose);

    impl HttpRequest for actix_web::HttpRequest {
        fn method(&self) -> http::Method {
            let method = self.method().as_str();
            http::Method::from_str(method).unwrap()
        }

        fn version(&self) -> http::Version {
            let version = self.version();
            if version == actix_web::http::Version::HTTP_10 {
                http::Version::HTTP_10
            } else if version == actix_web::http::Version::HTTP_11 {
                http::Version::HTTP_11
            } else if version == actix_web::http::Version::HTTP_2 {
                http::Version::HTTP_2
            } else if version == actix_web::http::Version::HTTP_3 {
                http::Version::HTTP_3
            } else {
                panic!("Unsupported HTTP version: {:?}", version)
            }
        }

        fn uri(&self) -> http::Uri {
            let uri = self.uri().to_string();
            uri.parse().unwrap()
        }

        fn headers(&self) -> Cow<http::HeaderMap> {
            let headers = self.headers();
            let mut res = http::HeaderMap::new();
            for (key, value) in headers.into_iter() {
                let key = http::header::HeaderName::from_bytes(key.as_str().as_bytes()).unwrap();
                let value = http::HeaderValue::from_bytes(value.as_bytes()).unwrap();
                res.insert(key, value);
            }
            Cow::Owned(res)
        }

        fn path(&self) -> &str {
            self.uri().path()
        }
    }

    impl<T> HttpJsonBody<T> for actix_web::web::Json<T> {
        fn get_body(self) -> T {
            self.0
        }
    }

    impl<T> HttpJsonQuery<T> for actix_web::web::Query<T> {
        fn get_query(self) -> T {
            self.0
        }
    }

    pub struct ActixIdentityCredential<T> {
        id: Arc<T>,
    }

    impl<T> ActixIdentityCredential<T>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        fn try_from_identity(identity: actix_identity::Identity) -> Result<Self, actix_web::Error> {
            let identity = identity;
            let id = match identity.id() {
                Ok(id) => id,
                Err(err) => {
                    return Err(err.into());
                }
            };
            let id = match T::from_str(&id) {
                Ok(id) => id,
                Err(err) => {
                    return Err(actix_web::error::ErrorBadRequest(format!(
                        "invalid id: {}",
                        err
                    )));
                }
            };

            Ok(Self { id: Arc::new(id) })
        }
    }

    impl<T> FromRequest for ActixIdentityCredential<T>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        type Error = actix_web::Error;

        type Future = std::future::Ready<Result<Self, Self::Error>>;

        fn from_request(
            req: &actix_web::HttpRequest,
            payload: &mut actix_web::dev::Payload,
        ) -> Self::Future {
            let identity = actix_identity::Identity::from_request(req, payload).into_inner();
            let identity = match identity {
                Ok(identity) => identity,
                Err(err) => {
                    return std::future::ready(Err(err));
                }
            };

            std::future::ready(ActixIdentityCredential::try_from_identity(identity))
        }
    }

    impl<T> HttpCredential for ActixIdentityCredential<T> {
        type Credential = super::IdCredential<T>;

        fn credential(&self) -> Self::Credential {
            super::IdCredential {
                id: self.id.clone(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code, unreachable_code, unused)]

    use super::*;

    struct HttpRequestImpl {}

    impl HttpRequest for HttpRequestImpl {
        fn method(&self) -> http::Method {
            http::Method::GET
        }

        fn version(&self) -> http::Version {
            http::Version::HTTP_11
        }

        fn uri(&self) -> http::Uri {
            unreachable!()
        }

        fn path(&self) -> &str {
            "/"
        }

        fn headers(&self) -> Cow<http::HeaderMap> {
            unreachable!()
        }
    }

    struct HttpJsonBodyImpl {}

    impl<T> HttpJsonBody<T> for HttpJsonBodyImpl {
        fn get_body(self) -> T {
            unreachable!()
        }
    }

    struct HttpJsonQueryImpl {}

    impl<T> HttpJsonQuery<T> for HttpJsonQueryImpl {
        fn get_query(self) -> T {
            unreachable!()
        }
    }

    struct HttpCredentialImpl {}

    impl HttpCredential for HttpCredentialImpl {
        type Credential = IdCredential<String>;

        fn credential(&self) -> Self::Credential {
            unreachable!()
        }
    }

    fn json_endpoint<R>(_req: R)
    where
        R: HttpJsonBody<u32> + HttpRequest,
    {
        unreachable!()
    }

    fn check_json_req() {
        type Req =
            HttpRequestCompose<HttpRequestImpl, HttpJsonBodyCompose<HttpJsonBodyImpl, ComposeNil>>;
        let req: Req = unreachable!();
        json_endpoint(req);
    }

    fn all_endpoints<R>(req: R)
    where
        R: HttpRequest + HttpJsonBody<u32> + HttpJsonQuery<u32> + HttpCredential,
    {
        unreachable!()
    }

    fn check_all() {
        type Req = HttpRequestCompose<
            HttpRequestImpl,
            HttpJsonBodyCompose<
                HttpJsonBodyImpl,
                HttpJsonQueryCompose<
                    HttpJsonQueryImpl,
                    HttpCredentialCompose<HttpCredentialImpl, ComposeNil>,
                >,
            >,
        >;

        let req: Req = unreachable!();
        all_endpoints(req);
    }
}
