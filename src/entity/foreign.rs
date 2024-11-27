use std::collections::HashSet;
use std::fmt::Debug;
use std::{borrow::Borrow, hash::Hash};

use indexmap::IndexSet;

use super::field::{Reset, Unchanged, Unloaded};
use super::SysId;

pub trait ForeignEntity: Borrow<Self::Id> {
    type Id: Clone + Eq + std::hash::Hash;
}

impl<T> ForeignEntity for T
where
    T: SysId,
{
    type Id = Self;
}

#[derive(Clone, PartialEq, Eq)]
pub enum ForeignEntities<C>
where
    C: ForeignContainer,
    <C as ForeignContainer>::Item: ForeignEntity,
{
    Unloaded,
    Unchanged(C),
    Reset(C),
    Changed {
        original: ForeignEntitiesState<C>,
        add: C,
        remove: HashSet<<<C as ForeignContainer>::Item as ForeignEntity>::Id>,
    },
}

impl<C> Debug for ForeignEntities<C>
where
    C: ForeignContainer + Debug,
    <C as ForeignContainer>::Item: ForeignEntity,
    HashSet<<<C as ForeignContainer>::Item as ForeignEntity>::Id>: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unloaded => write!(f, "Unloaded"),
            Self::Unchanged(arg0) => f.debug_tuple("Unchanged").field(arg0).finish(),
            Self::Reset(arg0) => f.debug_tuple("Reset").field(arg0).finish(),
            Self::Changed {
                original,
                add,
                remove,
            } => f
                .debug_struct("Changed")
                .field("original", original)
                .field("add", add)
                .field("remove", remove)
                .finish(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForeignEntitiesState<T> {
    Unloaded,
    Data(T),
}

pub trait ForeignContainer {
    type Item;

    fn new() -> Self;

    fn insert(&mut self, value: <Self as ForeignContainer>::Item) -> bool;

    fn remove<Q>(&mut self, value: &Q) -> bool
    where
        Self::Item: Borrow<Q>,
        Q: Hash + Eq;

    fn clear(&mut self);

    fn contains<Q>(&self, value: &Q) -> bool
    where
        Self::Item: Borrow<Q>,
        Q: Hash + Eq;

    fn is_empty(&self) -> bool;

    fn extend<I: IntoIterator<Item = Self::Item>>(&mut self, iter: I);
}

impl<C> ForeignEntities<C>
where
    C: ForeignContainer,
    <C as ForeignContainer>::Item: ForeignEntity,
{
    /// Returns the original foreign entities
    ///
    /// # Panics
    /// This function will panic if the field is not loaded.
    pub fn origin_value_ref(&self) -> &C {
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

    /// Returns the current foreign entities
    ///
    /// # Panics
    /// This function will panic if the field is not loaded.
    pub fn current_value(&self) -> C
    where
        C: Clone,
        C: IntoIterator<Item = <C as ForeignContainer>::Item>,
    {
        match self {
            ForeignEntities::Unloaded => {
                panic!("Field is not loaded. Type = {}", std::any::type_name::<C>())
            }
            ForeignEntities::Unchanged(v) => v.clone(),
            ForeignEntities::Reset(v) => v.clone(),
            ForeignEntities::Changed {
                original,
                add,
                remove,
            } => match original {
                ForeignEntitiesState::Unloaded => {
                    panic!("Field is not loaded. Type = {}", std::any::type_name::<C>())
                }
                ForeignEntitiesState::Data(v) => {
                    let mut container = v.clone();
                    for c in remove {
                        container.remove(&c);
                    }

                    container.extend(add.clone());

                    container
                }
            },
        }
    }
}

impl<C> ForeignEntities<C>
where
    C: ForeignContainer,
    <C as ForeignContainer>::Item: ForeignEntity,
{
    pub fn add(&mut self, value: <C as ForeignContainer>::Item) {
        match self {
            ForeignEntities::Unloaded => {
                *self = ForeignEntities::Changed {
                    original: ForeignEntitiesState::Unloaded,
                    add: {
                        let mut container = C::new();
                        container.insert(value);
                        container
                    },
                    remove: HashSet::new(),
                }
            }
            ForeignEntities::Unchanged(origin) => {
                // fixme: 或许应该移除这个检查，直接覆盖
                // 但考虑到写数据库时默认会使用 do nothing on conflict 的策略，这里即使覆盖也无法写进数据库
                let id: &<<C as ForeignContainer>::Item as ForeignEntity>::Id = value.borrow();
                if origin.contains(&id) {
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
                    remove: HashSet::new(),
                }
            }
            ForeignEntities::Reset(r) => {
                r.insert(value);
            }
            ForeignEntities::Changed {
                original: _,
                add,
                remove,
            } => {
                let id: &<<C as ForeignContainer>::Item as ForeignEntity>::Id = value.borrow();
                if remove.remove(&id) {
                    return;
                }
                add.insert(value);
            }
        }
    }

    pub fn remove(&mut self, value: <<C as ForeignContainer>::Item as ForeignEntity>::Id) {
        match self {
            ForeignEntities::Unloaded => {
                *self = ForeignEntities::Changed {
                    original: ForeignEntitiesState::Unloaded,
                    add: C::new(),
                    remove: {
                        let mut container = HashSet::new();
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
                        let mut container = HashSet::new();
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
                add,
                remove,
            } => {
                if add.remove(&value) {
                    return;
                }
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

impl<T> ForeignContainer for HashSet<T>
where
    T: Hash + Eq,
{
    type Item = T;

    fn new() -> Self {
        Self::new()
    }

    fn insert(&mut self, value: <Self as ForeignContainer>::Item) -> bool {
        self.insert(value)
    }

    fn remove<Q>(&mut self, value: &Q) -> bool
    where
        Self::Item: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.remove(value)
    }

    fn clear(&mut self) {
        self.clear();
    }

    fn contains<Q>(&self, value: &Q) -> bool
    where
        Self::Item: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.contains(value)
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn extend<I: IntoIterator<Item = Self::Item>>(&mut self, iter: I) {
        Extend::extend(self, iter);
    }
}
impl<T> ForeignContainer for IndexSet<T>
where
    T: Hash + Eq,
{
    type Item = T;

    fn new() -> Self {
        Self::new()
    }

    fn insert(&mut self, value: <Self as ForeignContainer>::Item) -> bool {
        self.insert(value)
    }

    fn clear(&mut self) {
        IndexSet::clear(self);
    }

    fn contains<Q>(&self, value: &Q) -> bool
    where
        Self::Item: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.contains(value)
    }

    fn is_empty(&self) -> bool {
        IndexSet::is_empty(self)
    }

    fn extend<I: IntoIterator<Item = Self::Item>>(&mut self, iter: I) {
        Extend::extend(self, iter);
    }

    fn remove<Q>(&mut self, value: &Q) -> bool
    where
        Self::Item: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.shift_remove(value)
    }
}

impl<C> Reset<ForeignEntities<C>> for C
where
    C: ForeignContainer,
    <C as ForeignContainer>::Item: ForeignEntity,
{
    fn reset(self) -> ForeignEntities<C> {
        ForeignEntities::Reset(self)
    }
}

impl<C> Unloaded for ForeignEntities<C>
where
    C: ForeignContainer,
    <C as ForeignContainer>::Item: ForeignEntity,
{
    fn unloaded() -> Self {
        ForeignEntities::Unloaded
    }
}

impl<C> Unchanged<ForeignEntities<C>> for C
where
    C: ForeignContainer,
    <C as ForeignContainer>::Item: ForeignEntity,
{
    fn unchanged(self) -> ForeignEntities<C> {
        ForeignEntities::Unchanged(self)
    }
}
