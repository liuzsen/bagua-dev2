use std::{collections::HashSet, hash::Hash};

use indexmap::IndexSet;

use super::field::{Reset, Unchanged, Unloaded};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForeignEntities<Container> {
    Unloaded,
    Unchanged(Container),
    Reset(Container),
    Changed {
        original: ForeignEntitiesState<Container>,
        add: Container,
        remove: Container,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForeignEntitiesState<T> {
    Unloaded,
    Data(T),
}

pub trait ForeignContainer: IntoIterator {
    fn new() -> Self;

    fn insert(&mut self, value: <Self as IntoIterator>::Item) -> bool;

    fn remove(&mut self, value: &<Self as IntoIterator>::Item) -> bool;

    fn clear(&mut self);

    fn contains(&self, value: &<Self as IntoIterator>::Item) -> bool;
}

impl<C> ForeignEntities<C> {
    pub fn value_ref(&self) -> &C {
        match self {
            ForeignEntities::Unloaded => {
                panic!("Field is not loaded. Type = {}", std::any::type_name::<C>())
            }
            ForeignEntities::Unchanged(v) => v,
            ForeignEntities::Reset(v) => v,
            ForeignEntities::Changed {
                original,
                add: _,
                remove: _,
            } => match original {
                ForeignEntitiesState::Unloaded => {
                    panic!("Field is not loaded. Type = {}", std::any::type_name::<C>())
                }
                ForeignEntitiesState::Data(v) => v,
            },
        }
    }
}

impl<C> ForeignEntities<C>
where
    C: ForeignContainer,
{
    pub fn add(&mut self, value: <C as IntoIterator>::Item) {
        match self {
            ForeignEntities::Unloaded => {
                *self = ForeignEntities::Changed {
                    original: ForeignEntitiesState::Unloaded,
                    add: {
                        let mut container = C::new();
                        container.insert(value);
                        container
                    },
                    remove: C::new(),
                }
            }
            ForeignEntities::Unchanged(origin) => {
                if origin.contains(&value) {
                    return;
                }

                let origin = std::mem::replace(origin, C::new());

                *self = ForeignEntities::Changed {
                    original: ForeignEntitiesState::Data(origin),
                    add: {
                        let mut container = C::new();
                        container.insert(value);
                        container
                    },
                    remove: C::new(),
                }
            }
            ForeignEntities::Reset(r) => {
                r.insert(value);
            }
            ForeignEntities::Changed {
                original: _,
                add,
                remove: _,
            } => {
                add.insert(value);
            }
        }
    }

    pub fn remove(&mut self, value: <C as IntoIterator>::Item) {
        match self {
            ForeignEntities::Unloaded => {
                *self = ForeignEntities::Changed {
                    original: ForeignEntitiesState::Unloaded,
                    add: C::new(),
                    remove: {
                        let mut container = C::new();
                        container.insert(value);
                        container
                    },
                }
            }
            ForeignEntities::Unchanged(origin) => {
                if !origin.contains(&value) {
                    return;
                }

                let origin = std::mem::replace(origin, C::new());

                *self = ForeignEntities::Changed {
                    original: ForeignEntitiesState::Data(origin),
                    add: C::new(),
                    remove: {
                        let mut container = C::new();
                        container.insert(value);
                        container
                    },
                }
            }
            ForeignEntities::Reset(r) => {
                r.remove(&value);
            }
            ForeignEntities::Changed {
                original: _,
                add: _,
                remove,
            } => {
                remove.insert(value);
            }
        }
    }

    pub fn reset(&mut self, value: C) {
        *self = ForeignEntities::Reset(value);
    }

    pub fn update_value(&mut self, value: Option<C>) {
        if let Some(value) = value {
            self.reset(value);
        }
    }
}

impl<C> Reset<ForeignEntities<C>> for C {
    fn reset(self) -> ForeignEntities<C> {
        ForeignEntities::Reset(self)
    }
}

impl<C> Unloaded for ForeignEntities<C> {
    fn unloaded() -> Self {
        ForeignEntities::Unloaded
    }
}

impl<C> Unchanged<ForeignEntities<C>> for C {
    fn unchanged(self) -> ForeignEntities<C> {
        ForeignEntities::Unchanged(self)
    }
}

impl<T> ForeignContainer for IndexSet<T>
where
    T: Hash + Eq,
{
    fn new() -> Self {
        Self::new()
    }

    fn insert(&mut self, value: <Self as IntoIterator>::Item) -> bool {
        IndexSet::insert(self, value)
    }

    fn remove(&mut self, value: &<Self as IntoIterator>::Item) -> bool {
        IndexSet::shift_remove(self, value)
    }

    fn clear(&mut self) {
        IndexSet::clear(self);
    }

    fn contains(&self, value: &<Self as IntoIterator>::Item) -> bool {
        IndexSet::contains(self, value)
    }
}

impl<T> ForeignContainer for HashSet<T>
where
    T: Hash + Eq,
{
    fn new() -> Self {
        Self::new()
    }

    fn insert(&mut self, value: <Self as IntoIterator>::Item) -> bool {
        HashSet::insert(self, value)
    }

    fn remove(&mut self, value: &<Self as IntoIterator>::Item) -> bool {
        HashSet::remove(self, value)
    }

    fn clear(&mut self) {
        HashSet::clear(self);
    }

    fn contains(&self, value: &<Self as IntoIterator>::Item) -> bool {
        HashSet::contains(self, value)
    }
}
