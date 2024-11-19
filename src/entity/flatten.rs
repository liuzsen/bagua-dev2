use std::ops::{Deref, DerefMut};

use super::{
    field::{Reset, Unchanged, Unloaded},
    GuardedStruct,
};

pub struct FlattenStruct<T: GuardedStruct>(T);

impl<T: GuardedStruct> FlattenStruct<T> {
    pub fn new(entity: T) -> Self {
        Self(entity)
    }
}

impl<T: GuardedStruct> Deref for FlattenStruct<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: GuardedStruct> DerefMut for FlattenStruct<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: GuardedStruct> Reset<FlattenStruct<T>> for T {
    fn reset(self) -> FlattenStruct<T> {
        FlattenStruct(self)
    }
}

impl<T: GuardedStruct> Unchanged<FlattenStruct<T>> for T {
    fn unchanged(self) -> FlattenStruct<T> {
        FlattenStruct(self)
    }
}

impl<T: GuardedStruct> Unloaded for FlattenStruct<T>
where
    T: Unloaded,
{
    fn unloaded() -> Self {
        FlattenStruct(T::unloaded())
    }
}
