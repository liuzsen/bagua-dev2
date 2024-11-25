use std::borrow::Cow;

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
    fn json_body(self) -> T;
}

pub trait HttpJsonQuery<T> {
    fn json_query(&self) -> T;
}

pub struct Credential {
    pub id: String,
    pub role: Cow<'static, str>,
}

pub trait HttpCredential {
    fn credential(&self) -> Credential;

    fn parse_id<T>(&self) -> Result<T, <T as std::str::FromStr>::Err>
    where
        T: std::str::FromStr,
    {
        let id = &self.credential().id;
        id.parse()
    }

    fn parse_role<T>(&self) -> Result<T, <T as std::str::FromStr>::Err>
    where
        T: std::str::FromStr,
    {
        let role = &self.credential().role;
        role.parse()
    }
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
    fn json_body(self) -> T {
        self.this.json_body()
    }
}

impl<A, B, T> HttpJsonQuery<T> for HttpJsonQueryCompose<A, B>
where
    A: HttpJsonQuery<T>,
{
    fn json_query(&self) -> T {
        self.this.json_query()
    }
}

impl<A, B> HttpCredential for HttpCredentialCompose<A, B>
where
    A: HttpCredential,
{
    fn credential(&self) -> Credential {
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

impl_http_request_for_that!(HttpJsonBodyCompose);
impl_http_request_for_that!(HttpJsonQueryCompose);
impl_http_request_for_that!(HttpCredentialCompose);

macro_rules! impl_http_json_body_for_that {
    ($compose:ident) => {
        impl<A, B, T> HttpJsonBody<T> for $compose<A, B>
        where
            B: HttpJsonBody<T>,
        {
            fn json_body(self) -> T {
                self.that.json_body()
            }
        }
    };
}

impl_http_json_body_for_that!(HttpRequestCompose);
impl_http_json_body_for_that!(HttpJsonQueryCompose);
impl_http_json_body_for_that!(HttpCredentialCompose);

macro_rules! impl_http_json_query_for_that {
    ($compose:ident) => {
        impl<A, B, T> HttpJsonQuery<T> for $compose<A, B>
        where
            B: HttpJsonQuery<T>,
        {
            fn json_query(&self) -> T {
                self.that.json_query()
            }
        }
    };
}

impl_http_json_query_for_that!(HttpRequestCompose);
impl_http_json_query_for_that!(HttpJsonBodyCompose);
impl_http_json_query_for_that!(HttpCredentialCompose);

macro_rules! impl_credential_for_that {
    ($compose:ident) => {
        impl<A, B> HttpCredential for $compose<A, B>
        where
            B: HttpCredential,
        {
            fn credential(&self) -> Credential {
                self.that.credential()
            }
        }
    };
}

impl_credential_for_that!(HttpRequestCompose);
impl_credential_for_that!(HttpJsonBodyCompose);
impl_credential_for_that!(HttpJsonQueryCompose);

#[cfg(feature = "actix-web")]
mod impl_traits {
    use std::{borrow::Cow, str::FromStr};

    use super::HttpRequest;

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
            todo!()
        }

        fn path(&self) -> &str {
            "/"
        }

        fn headers(&self) -> Cow<http::HeaderMap> {
            todo!()
        }
    }

    struct HttpJsonBodyImpl {}

    impl<T> HttpJsonBody<T> for HttpJsonBodyImpl {
        fn json_body(self) -> T {
            todo!()
        }
    }

    struct HttpJsonQueryImpl {}

    impl<T> HttpJsonQuery<T> for HttpJsonQueryImpl {
        fn json_query(&self) -> T {
            todo!()
        }
    }

    struct HttpCredentialImpl {}

    impl HttpCredential for HttpCredentialImpl {
        fn credential(&self) -> Credential {
            todo!()
        }
    }

    fn json_endpoint<R>(_req: R)
    where
        R: HttpJsonBody<u32> + HttpRequest,
    {
        todo!()
    }

    fn check_json_req() {
        type Req =
            HttpRequestCompose<HttpRequestImpl, HttpJsonBodyCompose<HttpJsonBodyImpl, ComposeNil>>;
        let req: Req = todo!();
        json_endpoint(req);
    }

    fn all_endpoints<R>(req: R)
    where
        R: HttpRequest + HttpJsonBody<u32> + HttpJsonQuery<u32> + HttpCredential,
    {
        todo!()
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

        let req: Req = todo!();
        all_endpoints(req);
    }
}
