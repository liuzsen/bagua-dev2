use super::{Entity, GuardedStruct, SysId};

pub trait Model {
    type Entity: Entity;

    fn build_entity(self) -> Self::Entity
    where
        <Self::Entity as Entity>::SysId: SysId;
}

pub trait ModelWithId {
    type Entity: Entity;

    fn build_entity(self, id: <Self::Entity as Entity>::SysId) -> Self::Entity;
}

pub trait GuardedModel {
    type GuardedStruct: GuardedStruct;

    fn build_entity(self) -> Self::GuardedStruct;
}
