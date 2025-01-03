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

pub mod biz_err {
    use std::borrow::Cow;

    use http::StatusCode;

    #[derive(Debug)]
    pub struct BizError {
        pub biz_code: u32,
        pub http_status: StatusCode,
        pub message: Cow<'static, str>,
    }

    impl BizError {
        pub const fn new(http_status: u16, biz_code: u32, message: &'static str) -> Self {
            Self {
                biz_code,
                http_status: u16_to_status_code(http_status),
                message: std::borrow::Cow::Borrowed(message),
            }
        }

        pub fn with_context<T>(mut self, ctx: T) -> Self
        where
            T: AsRef<str>,
        {
            let msg = self.message.to_mut();
            msg.push_str(": ");
            msg.push_str(ctx.as_ref());

            self
        }
    }

    /// Convert u16 to http status code at compile time
    ///
    /// Don't modify this function by hand as it was generated by a py script
    pub const fn u16_to_status_code(code: u16) -> StatusCode {
        match code {
            100 => StatusCode::CONTINUE,
            101 => StatusCode::SWITCHING_PROTOCOLS,
            102 => StatusCode::PROCESSING,
            200 => StatusCode::OK,
            201 => StatusCode::CREATED,
            202 => StatusCode::ACCEPTED,
            203 => StatusCode::NON_AUTHORITATIVE_INFORMATION,
            204 => StatusCode::NO_CONTENT,
            205 => StatusCode::RESET_CONTENT,
            206 => StatusCode::PARTIAL_CONTENT,
            207 => StatusCode::MULTI_STATUS,
            208 => StatusCode::ALREADY_REPORTED,
            226 => StatusCode::IM_USED,
            300 => StatusCode::MULTIPLE_CHOICES,
            301 => StatusCode::MOVED_PERMANENTLY,
            302 => StatusCode::FOUND,
            303 => StatusCode::SEE_OTHER,
            304 => StatusCode::NOT_MODIFIED,
            305 => StatusCode::USE_PROXY,
            307 => StatusCode::TEMPORARY_REDIRECT,
            308 => StatusCode::PERMANENT_REDIRECT,
            400 => StatusCode::BAD_REQUEST,
            401 => StatusCode::UNAUTHORIZED,
            402 => StatusCode::PAYMENT_REQUIRED,
            403 => StatusCode::FORBIDDEN,
            404 => StatusCode::NOT_FOUND,
            405 => StatusCode::METHOD_NOT_ALLOWED,
            406 => StatusCode::NOT_ACCEPTABLE,
            407 => StatusCode::PROXY_AUTHENTICATION_REQUIRED,
            408 => StatusCode::REQUEST_TIMEOUT,
            409 => StatusCode::CONFLICT,
            410 => StatusCode::GONE,
            411 => StatusCode::LENGTH_REQUIRED,
            412 => StatusCode::PRECONDITION_FAILED,
            413 => StatusCode::PAYLOAD_TOO_LARGE,
            414 => StatusCode::URI_TOO_LONG,
            415 => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            416 => StatusCode::RANGE_NOT_SATISFIABLE,
            417 => StatusCode::EXPECTATION_FAILED,
            418 => StatusCode::IM_A_TEAPOT,
            421 => StatusCode::MISDIRECTED_REQUEST,
            422 => StatusCode::UNPROCESSABLE_ENTITY,
            423 => StatusCode::LOCKED,
            424 => StatusCode::FAILED_DEPENDENCY,
            426 => StatusCode::UPGRADE_REQUIRED,
            428 => StatusCode::PRECONDITION_REQUIRED,
            429 => StatusCode::TOO_MANY_REQUESTS,
            431 => StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            451 => StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            500 => StatusCode::INTERNAL_SERVER_ERROR,
            501 => StatusCode::NOT_IMPLEMENTED,
            502 => StatusCode::BAD_GATEWAY,
            503 => StatusCode::SERVICE_UNAVAILABLE,
            504 => StatusCode::GATEWAY_TIMEOUT,
            505 => StatusCode::HTTP_VERSION_NOT_SUPPORTED,
            506 => StatusCode::VARIANT_ALSO_NEGOTIATES,
            507 => StatusCode::INSUFFICIENT_STORAGE,
            508 => StatusCode::LOOP_DETECTED,
            510 => StatusCode::NOT_EXTENDED,
            511 => StatusCode::NETWORK_AUTHENTICATION_REQUIRED,
            _ => panic!("invalid http status code"),
        }
    }
}

/// Macro to generate http api
///
/// # Example
///
/// ```rust
/// use bagua::http_api;
///
/// http_api!(post_create; post::create: HttpJsonBody + HttpCredential);
/// ```
#[macro_export]
macro_rules! http_api {
    ($fn_name:ident $(($(_: $extractor_ty:ty)*))?, $Va:ident $(::$Vb:ident)*) => {
        pub async fn $fn_name($($(_: $extractor_ty,)*)? )
        -> common::http::HttpApiResponse<crate::adapters::api_http::$Va $(::$Vb)*::Response> {
            use crate::infrastructure::types:: $Va $(::$Vb)* ::{Adapter, UseCase};
            use crate::infrastructure::types::TxnManager;
            use bagua::provider::Provider;
            use bagua::usecase::TxnUseCase;
            use common::http::Adapter as _;

            type UC = TxnUseCase<TxnManager, UseCase>;

            let uc = match UC::provide() {
                Ok(uc) => uc,
                Err(err) => {
                    return From::from(err);
                }
            };

            let mut adapter = Adapter {};
            adapter.run((), uc).await
        }
    };

    ($fn_name:ident $(($(_: $extractor_ty:ty)*))?, $Va:ident $(::$Vb:ident)* : $bound1:ident $(<$generic1:ty>)? $(+ $bounds:ident $(<$generics:ty>)?)*) => {
        pub async fn $fn_name($($(_: $extractor_ty,)*)? req: http_api!(@compose $Va $(::$Vb)*; $bound1 $(<$generic1>)?, $($bounds $(<$generics>)?),*))
        -> common::http::HttpApiResponse<crate::adapters::api_http::$Va $(::$Vb)*::Response> {
            use crate::infrastructure::types:: $Va $(::$Vb)* ::{Adapter, UseCase};
            use crate::infrastructure::types::TxnManager;
            use bagua::provider::Provider;
            use bagua::usecase::TxnUseCase;
            use common::http::Adapter as _;

            type UC = TxnUseCase<TxnManager, UseCase>;

            let uc = match UC::provide() {
                Ok(uc) => uc,
                Err(err) => {
                    return From::from(err);
                }
            };

            let mut adapter = Adapter {};
            adapter.run(req, uc).await
        }
    };

    (@compose $Va:ident $(::$Vb:ident)*; HttpRequest, $($bounds:ident $(<$generics:ty>)?),* $(,)*) => {
        bagua::http::HttpRequestCompose<
            crate::infrastructure::types::HttpRequest,
            http_api!(@compose $Va $(::$Vb)*; $($bounds $(<$generics>)?),* ,),
        >
    };

    (@compose $Va:ident $(::$Vb:ident)*; HttpCredential, $($bounds:ident $(<$generics:ty>)?),* $(,)*) => {
        bagua::http::HttpCredentialCompose<
            crate::infrastructure::types::HttpCredential,
            http_api!(@compose $Va $(::$Vb)*; $($bounds $(<$generics>)?),* ,),
        >
    };

    (@compose $Va:ident $(::$Vb:ident)*;  HttpJsonQuery, $($bounds:ident $(<$generics:ty>)?),* $(,)*) => {
        bagua::http::HttpJsonQueryCompose<
            crate::infrastructure::types::HttpJsonQuery<
                crate::adapters::api_http:: $Va $(::$Vb)* ::Request,
            >,
            http_api!(@compose $Va $(::$Vb)*; $($bounds $(<$generics>)?),* ,),
        >
    };

    (@compose $Va:ident $(::$Vb:ident)*;  HttpJsonQuery <$generic:ty>, $($bounds:ident $(<$generics:ty>)?),* $(,)*) => {
        bagua::http::HttpJsonQueryCompose<
            crate::infrastructure::types::HttpJsonQuery<$generic>,
            http_api!(@compose $Va $(::$Vb)*; $($bounds $(<$generics>)?),* ,),
        >
    };

    (@compose $Va:ident $(::$Vb:ident)*;  HttpJsonBody, $($bounds:ident $(<$generics:ty>)?),* $(,)*) => {
        bagua::http::HttpJsonBodyCompose<
            crate::infrastructure::types::HttpJsonBody<
                crate::adapters::api_http:: $Va $(::$Vb)* ::Request,
            >,
            http_api!(@compose $Va $(::$Vb)*; $($bounds $(<$generics>)?),* ,),
        >
    };

    (@compose $Va:ident $(::$Vb:ident)*;  HttpJsonBody <$generic:ty>, $($bounds:ident $(<$generics:ty>)?),* $(,)*) => {
        bagua::http::HttpJsonBodyCompose<
            crate::infrastructure::types::HttpJsonBody<$generic>,
            http_api!(@compose $Va $(::$Vb)*; $($bounds $(<$generics>)?),* ,),
        >
    };

    (@compose $Va:ident $(::$Vb:ident)*;  $bound1:ident $(<$generic:ty>)?, $($bounds:ident $(<$generics:ty>)?),* $(,)*) => {
        paste::paste! {
            bagua::http::[<$bound1 Compose>]<
                crate::infrastructure::types::$bound1 $(<$generic>)?,
                http_api!(@compose $Va $(::$Vb)*; $($bounds $(<$generics>)?),* ,),
            >
        }
    };

    (@compose $Va:ident $(::$Vb:ident)*; $(,)*) => {
        bagua::http::ComposeNil
    };
}

/// Macro to generate actix route
///
/// # Example
///
/// ```rust
/// pub fn route(cfg: &mut web::ServiceConfig) {
///     let mw1 = actix_web::middleware::from_fn(my_middleware);
///     let mw2 = actix_web::middleware::from_fn(my_middleware);
///     let mw3 = actix_web::middleware::from_fn(my_middleware);
///
///     bagua::actix_route!(
///         router = cfg;
///
///         {
///             "ping" GET => ping,
///         }
///
///         "admin" (mw: mw1) {
///             "aa" (mw: mw2)  {
///                 "bb"      POST    => ping,
///
///                 "ccc" (mw: mw3) {
///                     "aa"      POST    => ping |
///                             GET     => ping |
///                             DELETE  => ping,
///                     "bb"      POST    => ping,
///                 }
///             }
///         }
///
///         "api" {
///             "aa"      POST    => ping,
///         }
///     );
///
///     async fn my_middleware(
///         req: actix_web::dev::ServiceRequest,
///         next: actix_web::middleware::Next<impl actix_web::body::MessageBody>,
///     ) -> Result<actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>, actix_web::Error>
///     {
///         next.call(req).await
///     }
///
///     async fn ping() -> &'static str {
///         "pong"
///     }
/// }
/// ```

#[macro_export]
macro_rules! actix_route {
    (router = $router:expr; $($scope:literal $((mw: $mw:expr))? { $($scope_body:tt)* })*) => {
        $crate::actix_route!(
            @scope $router, $($scope $((mw: $mw))? { $($scope_body)* })*
        )
    };

    (router = $router:expr; {$($name:literal $($method:ident => $handler:path)|+  ),* $(,)?} $($scope:literal $((mw: $mw:expr))? { $($scope_body:tt)* })*) => {
        {
           let router = $crate::attach_resource! {
                $router,
                {
                    $($name $($method => $handler)|+,)*
                }
            };
            $crate::actix_route!(
                @scope router, $($scope $((mw: $mw))? { $($scope_body)* })*
            )
        }
    };

    (@scope $super_scope:expr, $scope:literal $((mw: $mw:expr))? {$($scope_body:tt)*} $($tt:tt)*) => {
        $crate::actix_route!(
            @scope
            $super_scope.service(
                $crate::attach_resource!{
                    actix_web::web::scope($scope),
                    {
                        $($scope_body)*
                    }
                }  $(.wrap($mw))?
            ),
            $($tt)*
        )
    };

    (@scope $super_scope:expr $(,)*) => {
           $super_scope
    };
}

#[macro_export]
macro_rules! attach_resource {
    ($scope:expr, { $name:literal $($method:ident => $handler:path)|+ , $($scope_tail:tt)* }) => {
        $crate::attach_resource!{
            $scope.service($crate::resource!($name $($method => $handler,)+)),
            {$($scope_tail)*}
        }
    };

    ($scope:expr, {$inner_scope:literal $((mw: $mw:expr))? {$($inner_body:tt)*} $($inner_tail:tt)*} ) => {
        $crate::actix_route!(@scope $scope, $inner_scope $((mw: $mw))? {$($inner_body)*} $($inner_tail)* )
    };

    ($scope:expr, {$(,)*}) => {
        $scope
    };
}

#[macro_export]
macro_rules! resource {
    ($name:literal $($method:ident => $handler:expr,)*) => {
        $crate::resource!{@method
            actix_web::web::resource($name),
            $($method => $handler,)*
        }
    };

    (@method $resource:expr, GET => $handler:expr, $($tt:tt)*) => {{
        $crate::resource!(@method $resource.get($handler), $($tt)*)
    }};

    (@method $resource:expr, POST => $handler:expr, $($tt:tt)*) => {{
        $crate::resource!(@method $resource.post($handler), $($tt)*)
    }};

    (@method $resource:expr, DELETE => $handler:expr, $($tt:tt)*) => {{
        $crate::resource!(@method $resource.delete($handler), $($tt)*)
    }};

    (@method $resource:expr, PUT => $handler:expr, $($tt:tt)*) => {{
        $crate::resource!(@method $resource.put($handler), $($tt)*)
    }};

    (@method $resource:expr, PATCH => $handler:expr, $($tt:tt)*) => {{
        $crate::resource!(@method $resource.patch($handler), $($tt)*)
    }};

    (@method $resource:expr, HEAD => $handler:expr, $($tt:tt)*) => {{
        $crate::resource!(@method $resource.head($handler), $($tt)*)
    }};

    (@method $resource:expr, OPTIONS => $handler:expr, $($tt:tt)*) => {{
        $crate::resource!(@method $resource.options($handler), $($tt)*)
    }};

    (@method $resource:expr $(,)*) => {{
        $resource
    }};
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
