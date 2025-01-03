use std::collections::HashSet;

use crate::entity::{
    foreign::{ForeignContainer, ForeignEntities, ForeignEntity},
    subset::Subset,
    Entity,
};

/// Check UpdateEffect and return if not ok
#[macro_export]
macro_rules! check_update_effect {
    ($effect:expr) => {
        let effect: ::bagua::repository::UpdateEffect = $effect;
        if !effect.is_ok() {
            return Ok(effect);
        }
    };
}

/// Check SaveEffect and return if not ok
#[macro_export]
macro_rules! check_save_effect {
    ($effect:expr) => {
        let effect: ::bagua::repository::SaveEffect = $effect;
        if !effect.is_ok() {
            return Ok(effect);
        }
    };
}

/// Check DeleteEffect and return if not ok
#[macro_export]
macro_rules! check_delete_effect {
    ($effect:expr) => {
        let effect: ::bagua::repository::DeleteEffect = $effect;
        if !effect.is_ok() {
            return Ok(effect);
        }
    };
}

pub trait Repository<E: Entity> {
    async fn find<S, I>(&mut self, id: I) -> anyhow::Result<Option<E>>
    where
        S: Subset<Entity = E>,
        Self: SubsetLoader<S>,
        for<'a> E::Id<'a>: From<I>,
    {
        let subset = self.load(id).await?;
        Ok(subset.map(|s| s.to_entity()))
    }

    async fn find_batch<S, C>(&mut self, condition: C) -> anyhow::Result<Vec<E>>
    where
        S: Subset<Entity = E>,
        Self: BatchSubsetLoader<C, S>,
    {
        let subset = self.load_batch(condition).await?;
        Ok(subset.into_iter().map(|s| s.to_entity()).collect())
    }

    async fn read<S, I>(&mut self, id: I) -> anyhow::Result<Option<E>>
    where
        S: Subset<Entity = E>,
        Self: SubsetReader<S>,
        for<'a> E::Id<'a>: From<I>,
    {
        let subset = SubsetReader::read(self, id).await?;
        Ok(subset.map(|s| s.to_entity()))
    }

    async fn read_batch<S, C>(&mut self, condition: C) -> anyhow::Result<Vec<E>>
    where
        S: Subset<Entity = E>,
        Self: BatchSubsetReader<C, S>,
    {
        let subset = BatchSubsetReader::read_batch(self, condition).await?;
        Ok(subset.into_iter().map(|s| s.to_entity()).collect())
    }

    async fn save(&mut self, entity: &E) -> anyhow::Result<SaveEffect>;

    async fn update(&mut self, entity: &E) -> anyhow::Result<UpdateEffect>;

    async fn delete<I>(&mut self, id: I) -> anyhow::Result<DeleteEffect>
    where
        for<'a> E::Id<'a>: From<I>;

    async fn exists<I>(&mut self, id: I) -> anyhow::Result<bool>
    where
        for<'a> E::Id<'a>: From<I>;

    async fn fast_exists<I>(&mut self, id: I) -> anyhow::Result<FastExists>
    where
        for<'a> E::Id<'a>: From<I>,
    {
        let exists = self.exists(id).await?;
        if exists {
            Ok(FastExists::Yes)
        } else {
            Ok(FastExists::No)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FastExists {
    Yes,
    No,
    YesButNotSure,
    NoButNotSure,
}

pub trait SubsetLoader<S: Subset> {
    async fn load<I>(&mut self, id: I) -> anyhow::Result<Option<S>>
    where
        for<'a> <<S as Subset>::Entity as Entity>::Id<'a>: From<I>;
}

pub trait SubsetReader<S: Subset> {
    async fn read<I>(&mut self, id: I) -> anyhow::Result<Option<S>>
    where
        for<'a> <<S as Subset>::Entity as Entity>::Id<'a>: From<I>;
}

pub trait BatchSubsetLoader<C, S: Subset> {
    async fn load_batch(&mut self, condition: C) -> anyhow::Result<Vec<S>>;
}

pub trait BatchSubsetReader<C, S: Subset> {
    async fn read_batch(&mut self, condition: C) -> anyhow::Result<Vec<S>>;
}

#[must_use = "Save effect should be checked"]
pub enum SaveEffect {
    Ok,
    Conflict,
}

#[must_use = "Delete effect should be checked"]
pub enum DeleteEffect {
    Ok,
    NotFound,
}

#[must_use = "Update effect should be checked"]
pub enum UpdateEffect {
    Ok,
    Conflict,
    NotFound,
}

impl UpdateEffect {
    pub fn is_not_found(&self) -> bool {
        matches!(self, UpdateEffect::NotFound)
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, UpdateEffect::Ok)
    }

    pub fn is_effected(&self) -> bool {
        self.is_ok()
    }

    pub fn is_conflict(&self) -> bool {
        matches!(self, UpdateEffect::Conflict)
    }

    pub fn ignore_effect(self) {}
}

impl SaveEffect {
    pub fn is_conflict(&self) -> bool {
        matches!(self, SaveEffect::Conflict)
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, SaveEffect::Ok)
    }

    pub fn is_effected(&self) -> bool {
        self.is_ok()
    }

    pub fn ignore_effect(self) {}
}

impl DeleteEffect {
    pub fn is_not_found(&self) -> bool {
        matches!(self, DeleteEffect::NotFound)
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, DeleteEffect::Ok)
    }

    pub fn is_effected(&self) -> bool {
        self.is_ok()
    }

    pub fn ignore_effect(self) {}
}

pub trait ForeignEntitiesOperator<LocalId, C>
where
    C: ForeignContainer,
    <C as ForeignContainer>::Item: ForeignEntity,
    C: IntoIterator<Item = <C as ForeignContainer>::Item>,
    for<'a> &'a C: IntoIterator<Item = &'a <C as ForeignContainer>::Item>,
{
    async fn save_foreign(
        &mut self,
        id: &LocalId,
        foreign_entities: &ForeignEntities<C>,
    ) -> anyhow::Result<()> {
        match foreign_entities {
            ForeignEntities::Unloaded => {}
            ForeignEntities::Unchanged(_) => {}
            ForeignEntities::Reset(reset) => {
                self.clear_foreign(id).await?;
                self.add_foreign(id, reset).await?;
            }
            ForeignEntities::Changed {
                original: _,
                add,
                remove,
            } => {
                self.add_foreign(&id, add).await?;
                self.remove_foreign(&id, remove).await?;
            }
        }

        Ok(())
    }

    async fn clear_foreign(&mut self, id: &LocalId) -> anyhow::Result<()>;

    async fn remove_foreign(
        &mut self,
        id: &LocalId,
        foreign_entities: &HashSet<<<C as ForeignContainer>::Item as ForeignEntity>::Id>,
    ) -> anyhow::Result<()>;

    async fn add_foreign<'a, F>(
        &mut self,
        id: &'a LocalId,
        foreign_entities: F,
    ) -> anyhow::Result<()>
    where
        F: IntoIterator<Item = &'a <C as ForeignContainer>::Item>,
        <F as IntoIterator>::Item: 'a;
}
