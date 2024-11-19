use super::Entity;

pub trait Subset {
    type Entity: Entity;

    fn to_entity(self) -> Self::Entity;
}
