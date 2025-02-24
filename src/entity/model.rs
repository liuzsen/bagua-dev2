use super::{Entity, FieldGroup, SysId};

pub trait Model: Sized {
    type Entity: Entity;

    fn build_entity(self) -> Self::Entity
    where
        <Self::Entity as Entity>::SysId: SysId;
}

pub trait FieldGroupModel {
    type FieldGroup: FieldGroup;

    fn build_entity(self) -> Self::FieldGroup;
}
