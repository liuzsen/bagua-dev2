use bagua::HasChanged;

#[allow(unused)]
#[derive(HasChanged)]
struct AA {
    id: i32,
    a: Option<u8>,
    b: Option<u8>,
}

#[test]
fn t_has_changed() {
    let a = AA {
        id: 1,
        a: Some(1),
        b: None,
    };
    assert!(a.has_changed());

    let a = AA {
        id: 1,
        a: None,
        b: None,
    };
    assert!(!a.has_changed());
}
