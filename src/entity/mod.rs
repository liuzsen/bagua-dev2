use std::{fmt::Debug, hash::Hash};

use updater::Updater;

pub mod field;
pub mod flatten;
pub mod foreign;
pub mod model;
pub mod parent;
pub mod subset;
pub mod updater;

pub trait Entity: FieldGroup {
    type Id<'a>: Eq;

    type SysId: Eq;

    type BizIdFieldEnum: BizIdFieldEnum;
}

pub trait BizIdFieldEnum: Copy + Clone + Eq {
    fn field_name(self) -> &'static str;
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum NoBizIdField {}

impl BizIdFieldEnum for NoBizIdField {
    fn field_name(self) -> &'static str {
        match self {}
    }
}

pub trait SysId: Eq + Clone + Debug + Hash {
    fn generate() -> Self;
}

pub trait ChildEntity: Entity {
    type Parent: Entity;
}

pub trait FieldGroup {
    type Updater: Updater<FieldGroup = Self>;
    type SubsetFull;

    fn update_fields(&mut self, updater: Self::Updater);
}
