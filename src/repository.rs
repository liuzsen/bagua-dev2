use crate::entity::{subset::Subset, Entity};

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
