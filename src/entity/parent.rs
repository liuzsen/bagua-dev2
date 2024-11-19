use std::ops::{Deref, DerefMut};

use super::{
    field::{Reset, Unchanged},
    Entity,
};

pub struct ParentEntity<T>(T)
where
    T: Entity;

impl<T> ParentEntity<T>
where
    T: Entity,
{
    pub fn new(entity: T) -> Self {
        Self(entity)
    }
}

impl<T> Deref for ParentEntity<T>
where
    T: Entity,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ParentEntity<T>
where
    T: Entity,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Entity> Reset<ParentEntity<T>> for T {
    fn reset(self) -> ParentEntity<T> {
        ParentEntity::new(self)
    }
}

impl<T: Entity> Unchanged<ParentEntity<T>> for T {
    fn unchanged(self) -> ParentEntity<T> {
        ParentEntity::new(self)
    }
}
