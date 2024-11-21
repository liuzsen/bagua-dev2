use bagua::GuardedStruct;

#[GuardedStruct]
pub struct Permits {
    #[entity(flatten)]
    inner: Permits2,
}

#[GuardedStruct]
pub struct Permits2 {
    read: bool,
    write: bool,
    execute: bool,
}
