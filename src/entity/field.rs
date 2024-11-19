use std::{borrow::Cow, ops::Deref};

#[derive(Clone, PartialEq, Eq, Copy)]
pub enum Field<T> {
    Unloaded,
    Unchanged(T),
    Set(T),
}

impl<T> Deref for Field<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Field::Unloaded => panic!("Field is not loaded. Type = {}", std::any::type_name::<T>()),
            Field::Unchanged(v) => v,
            Field::Set(v) => v,
        }
    }
}

impl<T> Field<T> {
    pub fn to_set(&mut self) {
        let old = std::mem::replace(self, Field::Unloaded);
        match old {
            Field::Unloaded => {}
            Field::Unchanged(value) => *self = Field::Set(value),
            Field::Set(value) => *self = Field::Set(value),
        }
    }

    pub fn value_ref(&self) -> &T {
        match self {
            Field::Unloaded => panic!("Field is not loaded. Type = {}", std::any::type_name::<T>()),
            Field::Unchanged(v) => v,
            Field::Set(v) => v,
        }
    }

    pub fn value(self) -> T {
        match self {
            Field::Unloaded => panic!("Field is not loaded. Type = {}", std::any::type_name::<T>()),
            Field::Unchanged(v) => v,
            Field::Set(v) => v,
        }
    }

    pub fn value_ref_opt(&self) -> Option<&T> {
        match self {
            Field::Unloaded => None,
            Field::Unchanged(v) => Some(v),
            Field::Set(v) => Some(v),
        }
    }

    pub fn value_opt(self) -> Option<T> {
        match self {
            Field::Unloaded => None,
            Field::Unchanged(value) => Some(value),
            Field::Set(value) => Some(value),
        }
    }

    /// Set the value of the field.
    pub fn set(&mut self, value: T) {
        match self {
            Field::Unloaded => *self = Field::Set(value),
            Field::Unchanged(_) => *self = Field::Set(value),
            Field::Set(v) => *v = value,
        }
    }

    /// # Panic
    /// This function will panic if the field is not loaded.
    pub fn to_mut(&mut self) -> &mut T {
        match self {
            Field::Unloaded => panic!("Field is not loaded. Type = {}", std::any::type_name::<T>()),
            Field::Set(v) => v,
            Field::Unchanged(_) => {
                replace_with::replace_with_or_abort(self, |o| match o {
                    Field::Unchanged(o) => Field::Set(o),
                    _ => unreachable!(),
                });

                self.to_mut()
            }
        }
    }

    pub fn update_value(&mut self, value: Option<T>) {
        if let Some(value) = value {
            self.set(value);
        }
    }
}

pub trait FilledValue<'a, T, M> {
    fn value_ref(&'a self) -> T;
}

pub trait ConvertTo<T> {
    fn convert_to(&self) -> T;
}

pub struct MarkConvertTo;

impl<'a, T, O> FilledValue<'a, &'a O, MarkConvertTo> for Field<T>
where
    T: ConvertTo<&'a O>,
{
    fn value_ref(&'a self) -> &'a O {
        let value = self.value_ref();
        value.convert_to()
    }
}

pub struct MarkRef;

impl<'a, T, O> FilledValue<'a, &'a O, MarkRef> for Field<T>
where
    T: Deref<Target = O>,
{
    fn value_ref(&'a self) -> &'a O {
        self.value_ref()
    }
}

pub struct MarkCow;

impl<'a, T> FilledValue<'a, Cow<'a, T>, MarkCow> for Field<T>
where
    T: ToOwned,
{
    fn value_ref(&'a self) -> Cow<'a, T> {
        let value = self.value_ref();
        Cow::Borrowed(value)
    }
}

pub struct MarkCopy;

impl<'a, T> FilledValue<'a, T, MarkCopy> for Field<T>
where
    T: Copy,
{
    fn value_ref(&'a self) -> T {
        *self.value_ref()
    }
}

pub struct MarkAsRef;

impl<'a, T, Ref> FilledValue<'a, &'a Ref, MarkAsRef> for Field<T>
where
    T: AsRef<Ref>,
    Ref: ?Sized,
{
    fn value_ref(&'a self) -> &'a Ref {
        let value = self.value_ref();
        value.as_ref()
    }
}

pub trait Unchanged<Output> {
    fn unchanged(self) -> Output;
}

pub trait Unloaded {
    fn unloaded() -> Self;
}

pub trait Reset<Output> {
    fn reset(self) -> Output;
}

impl<T> Unchanged<T> for T {
    fn unchanged(self) -> T {
        self
    }
}

impl<T> Unchanged<Field<T>> for T {
    fn unchanged(self) -> Field<T> {
        Field::Unchanged(self)
    }
}

impl<T> Reset<Field<T>> for T {
    fn reset(self) -> Field<T> {
        Field::Set(self)
    }
}

impl<T> Unloaded for Field<T> {
    fn unloaded() -> Self {
        Field::Unloaded
    }
}
