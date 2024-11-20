use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
};

pub trait Provider: Sized + 'static {
    fn build(ctx: &mut ProviderContext) -> anyhow::Result<Self>;

    #[allow(dead_code)]
    fn provide() -> anyhow::Result<Self> {
        let mut ctx = ProviderContext::new();
        Self::build(&mut ctx)
    }

    fn provide_with(f: impl FnOnce(&mut ProviderContext)) -> anyhow::Result<Self> {
        let mut ctx = ProviderContext::new();
        f(&mut ctx);
        Self::build(&mut ctx)
    }
}

pub trait SingletonProvider: Provider + Clone + 'static {
    fn build_single(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
        if let Some(this) = ctx.get::<Self>() {
            return Ok(this.clone());
        }

        let this = Self::build(ctx)?;
        ctx.insert(this.clone());
        Ok(this)
    }
}

pub struct ProviderContext {
    map: HashMap<TypeId, Box<dyn Any>>,
}

impl ProviderContext {
    pub fn new() -> Self {
        ProviderContext {
            map: HashMap::new(),
        }
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref())
    }

    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.map
            .remove(&TypeId::of::<T>())
            .and_then(|boxed| downcast_owned(boxed))
    }

    pub fn insert<T: 'static>(&mut self, val: T) -> Option<T> {
        self.map
            .insert(TypeId::of::<T>(), Box::new(val))
            .and_then(downcast_owned)
    }

    pub fn with_instance<I: 'static>(mut self, instance: I) -> Self {
        self.insert(instance);
        self
    }

    pub fn build<T>(&mut self) -> anyhow::Result<T>
    where
        T: Provider,
    {
        Provider::build(self)
    }
}

fn downcast_owned<T: 'static>(boxed: Box<dyn Any>) -> Option<T> {
    boxed.downcast().ok().map(|boxed| *boxed)
}

impl<T> Provider for PhantomData<T>
where
    T: 'static,
{
    fn build(_ctx: &mut ProviderContext) -> anyhow::Result<Self> {
        Ok(PhantomData)
    }
}

impl Provider for () {
    fn build(_ctx: &mut ProviderContext) -> anyhow::Result<Self> {
        Ok(())
    }
}
