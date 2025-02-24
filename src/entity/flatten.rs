use std::ops::{Deref, DerefMut};

use super::{
    field::{Reset, Unchanged, Unloaded},
    FieldGroup,
};

pub struct FieldGroupWrapper<T: FieldGroup>(T);

impl<T: FieldGroup> FieldGroupWrapper<T> {
    pub fn new(entity: T) -> Self {
        Self(entity)
    }
}

impl<T: FieldGroup> Deref for FieldGroupWrapper<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: FieldGroup> DerefMut for FieldGroupWrapper<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: FieldGroup> Reset<FieldGroupWrapper<T>> for T {
    fn reset(self) -> FieldGroupWrapper<T> {
        FieldGroupWrapper(self)
    }
}

impl<T: FieldGroup> Unchanged<FieldGroupWrapper<T>> for T {
    fn unchanged(self) -> FieldGroupWrapper<T> {
        FieldGroupWrapper(self)
    }
}

impl<T: FieldGroup> Unloaded for FieldGroupWrapper<T>
where
    T: Unloaded,
{
    fn unloaded() -> Self {
        FieldGroupWrapper(T::unloaded())
    }
}
