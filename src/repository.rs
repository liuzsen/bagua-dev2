use std::collections::HashSet;

use crate::entity::{
    foreign::{ForeignContainer, ForeignEntities, ForeignEntity},
    subset::Subset,
    Entity,
};

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

    async fn save(&mut self, entity: &E) -> anyhow::Result<SaveEffect>;

    async fn update(&mut self, entity: &E) -> anyhow::Result<UpdateEffect>;

    async fn delete<I>(&mut self, id: I) -> anyhow::Result<DeleteEffect>
    where
        for<'a> E::Id<'a>: From<I>;

    async fn exists<I>(&mut self, id: I) -> anyhow::Result<bool>
    where
        for<'a> E::Id<'a>: From<I>;
}

pub trait SubsetLoader<S: Subset> {
    async fn load<I>(&mut self, id: I) -> anyhow::Result<Option<S>>
    where
        for<'a> <<S as Subset>::Entity as Entity>::Id<'a>: From<I>;
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

pub trait FromZeroOrOneEffect {
    fn from_zero_or_one(effect: usize) -> Self;
}

impl FromZeroOrOneEffect for SaveEffect {
    fn from_zero_or_one(effect: usize) -> Self {
        match effect {
            0 => SaveEffect::Ok,
            1 => SaveEffect::Conflict,
            _ => panic!("unexpected effect when saving: {}", effect),
        }
    }
}

impl FromZeroOrOneEffect for DeleteEffect {
    fn from_zero_or_one(effect: usize) -> Self {
        match effect {
            0 => DeleteEffect::Ok,
            1 => DeleteEffect::NotFound,
            _ => panic!("unexpected effect when deleting: {}", effect),
        }
    }
}

impl FromZeroOrOneEffect for UpdateEffect {
    fn from_zero_or_one(effect: usize) -> Self {
        match effect {
            0 => UpdateEffect::Ok,
            1 => UpdateEffect::NotFound,
            _ => panic!("unexpected effect when updating: {}", effect),
        }
    }
}

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

pub trait ForeignEntitiesOperator<LocalId, C>
where
    C: ForeignContainer,
    <C as ForeignContainer>::Item: ForeignEntity,
    C: IntoIterator<Item = <C as ForeignContainer>::Item>,
    for<'a> &'a C: IntoIterator<Item = &'a <C as ForeignContainer>::Item>,
{
    async fn init_foreign(
        &mut self,
        id: &LocalId,
        foreign_entities: &ForeignEntities<C>,
    ) -> anyhow::Result<()>;

    async fn update_foreign(
        &mut self,
        id: &LocalId,
        foreign_entities: &ForeignEntities<C>,
    ) -> anyhow::Result<UpdateEffect> {
        match foreign_entities {
            ForeignEntities::Unloaded => {}
            ForeignEntities::Unchanged(_) => {}
            ForeignEntities::Reset(reset) => {
                let effect = self.reset_foreign(id, reset).await?;
                if !effect.is_ok() {
                    return Ok(effect);
                };
            }
            ForeignEntities::Changed {
                original: _,
                add,
                remove,
            } => {
                let effect = self.add_foreign(&id, add).await?;
                if !effect.is_ok() {
                    return Ok(effect);
                };
                self.remove_foreign(&id, remove).await?;
            }
        }

        Ok(UpdateEffect::Ok)
    }

    async fn reset_foreign<'a>(
        &mut self,
        id: &'a LocalId,
        foreign_entities: &'a C,
    ) -> anyhow::Result<UpdateEffect> {
        self.clear_foreign(id).await?;

        let effect = self.add_foreign(id, foreign_entities).await?;
        if !effect.is_ok() {
            return Ok(effect);
        };

        Ok(UpdateEffect::Ok)
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
    ) -> anyhow::Result<UpdateEffect>
    where
        F: IntoIterator<Item = &'a <C as ForeignContainer>::Item>,
        <F as IntoIterator>::Item: 'a;
}
