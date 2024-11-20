#![allow(unused)]

use bagua::provider::Provider;
use macros::Provider;

#[derive(Provider)]
struct AA<T, T2> {
    #[provider(default)]
    a: T,
    #[provider(instance)]
    b: MyU8,

    t2: T2,
}

struct MyU8(u8);

#[test]
fn aa() {
    #[derive(Provider)]
    struct T2Provider {}

    type AAProvider = AA<(), T2Provider>;
    assert!(AAProvider::provide().is_err());

    let aa = AAProvider::provide_with(|ctx| {
        ctx.insert(MyU8(1));
    })
    .unwrap();

    assert_eq!(aa.b.0, 1);
}
