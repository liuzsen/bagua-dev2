use bagua::BizErrorEnum;

#[BizErrorEnum]
#[base_biz_code = 1000]
pub enum Error1 {}

#[test]
fn t1() {
    let all = Error1::all();
    assert!(all.is_empty())
}

#[BizErrorEnum]
#[base_biz_code = 1000]
#[default_http_status = 400]
#[err_path(bagua::http::biz_err::BizError)]
pub enum Error2 {
    UserNotFound,

    /// Manual description. User already exists
    #[http_status = 409]
    UserAlreadyExists,
}

#[test]
fn t2() {
    let all = Error2::all();
    assert_eq!(all.len(), 2);

    assert_eq!(Error2::UserNotFound.biz_code, 1001);
    assert_eq!(Error2::UserAlreadyExists.biz_code, 1002);

    assert_eq!(Error2::UserNotFound.http_status, 400);
    assert_eq!(Error2::UserAlreadyExists.http_status, 409);

    assert_eq!(Error2::UserNotFound.message, "User not found");
    assert_eq!(
        Error2::UserAlreadyExists.message,
        "Manual description. User already exists"
    );
}
