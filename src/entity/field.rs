use std::ops::Deref;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field<T> {
    Unloaded,
    Unchanged(T),
    Set(T),
}

impl<T> PartialEq<T> for Field<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &T) -> bool {
        let this = self.value_ref();
        this == other
    }
}

impl<T> PartialEq<&T> for Field<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &&T) -> bool {
        let this = self.value_ref();
        this == *other
    }
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

    pub fn changed_ref(&self) -> Option<&T> {
        match self {
            Field::Unloaded => None,
            Field::Unchanged(_) => None,
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

pub use field_value::*;
mod field_value {
    use std::borrow::Cow;

    use super::Field;
    use std::marker::PhantomData;

    #[test]
    #[ignore]
    fn check_convert() {
        #[allow(unused)]
        fn int(f: &Field<u32>) {
            let _: u32 = f.filled();
            let _: i32 = f.filled();
        }

        #[allow(unused)]
        fn str(f: &Field<String>) {
            let _: &str = f.filled();
            let _: &String = f.filled();
            let _: Cow<str> = f.filled();
        }
    }

    pub trait FiledValue<'a, T, M> {
        fn filled(&'a self) -> T;

        fn changed(&'a self) -> Option<T>;
    }

    pub trait ConvertTo<'a, T> {
        fn convert_to(&'a self) -> T;
    }

    macro_rules! convert_int {
        ($source:ty => $target:ident) => {
            impl<'a> ConvertTo<'a, $target> for $source {
                fn convert_to(&'a self) -> $target {
                    match $target::try_from(*self) {
                        Ok(v) => v,
                        Err(_) => {
                            panic!(
                                "Field value overflow. Type = {}, value = {self}",
                                std::any::type_name::<Self>()
                            )
                        }
                    }
                }
            }
        };
    }
    convert_int!(u64 => i64);
    convert_int!(u32 => i32);
    convert_int!(u16 => i16);
    convert_int!(u8 => i8);

    pub struct ComposeMark<A, B>(PhantomData<(A, B)>);

    pub struct MarkBorrow;
    pub struct MarkAsRef;
    pub struct MarkCopy;
    pub struct MarkConvertToRef;
    pub struct MarkConvertToValue;
    pub struct MarkCow;

    impl<'a, T> FiledValue<'a, &'a T, MarkBorrow> for Field<T> {
        fn filled(&'a self) -> &'a T {
            self.value_ref()
        }

        fn changed(&'a self) -> Option<&'a T> {
            self.changed_ref()
        }
    }

    pub struct MarkSelf;

    impl<'a, T> FiledValue<'a, &'a T, MarkSelf> for T {
        fn filled(&'a self) -> &'a T {
            self
        }

        fn changed(&'a self) -> Option<&'a T> {
            Some(self)
        }
    }

    impl<'a, T, Ref> FiledValue<'a, &'a Ref, MarkAsRef> for Field<T>
    where
        T: AsRef<Ref>,
        Ref: ?Sized,
    {
        fn filled(&'a self) -> &'a Ref {
            let value = self.value_ref();
            value.as_ref()
        }

        fn changed(&'a self) -> Option<&'a Ref> {
            let value = self.changed_ref();
            value.map(|v| v.as_ref())
        }
    }

    impl<'a, T> FiledValue<'a, T, MarkCopy> for Field<T>
    where
        T: Copy,
    {
        fn filled(&'a self) -> T {
            *self.value_ref()
        }

        fn changed(&'a self) -> Option<T> {
            self.value_opt()
        }
    }

    impl<'a, T, To> FiledValue<'a, &'a To, MarkConvertToRef> for Field<T>
    where
        T: ConvertTo<'a, &'a To>,
    {
        fn filled(&'a self) -> &'a To {
            let value = self.value_ref();
            value.convert_to()
        }

        fn changed(&'a self) -> Option<&'a To> {
            let value = self.changed_ref();
            value.map(|v| v.convert_to())
        }
    }

    impl<'a, T, To> FiledValue<'a, To, MarkConvertToValue> for Field<T>
    where
        T: ConvertTo<'a, To>,
    {
        fn filled(&'a self) -> To {
            self.value_ref().convert_to()
        }

        fn changed(&'a self) -> Option<To> {
            self.changed_ref().map(|v| v.convert_to())
        }
    }

    impl<'a, T, Ref> FiledValue<'a, Cow<'a, Ref>, ComposeMark<MarkAsRef, MarkCow>> for Field<T>
    where
        Ref: ToOwned + ?Sized,
        T: AsRef<Ref>,
    {
        fn filled(&'a self) -> Cow<'a, Ref> {
            let value = self.value_ref();
            Cow::Borrowed(value.as_ref())
        }

        fn changed(&'a self) -> Option<Cow<'a, Ref>> {
            let value = self.changed_ref();
            value.map(|v| Cow::Borrowed(v.as_ref()))
        }
    }

    impl<'a, T> FiledValue<'a, Cow<'a, T>, ComposeMark<MarkBorrow, MarkCow>> for Field<T>
    where
        T: ToOwned,
    {
        fn filled(&'a self) -> Cow<'a, T> {
            let value = self.value_ref();
            Cow::Borrowed(value)
        }

        fn changed(&'a self) -> Option<Cow<'a, T>> {
            self.changed_ref().map(Cow::Borrowed)
        }
    }

    impl<'a, T, To> FiledValue<'a, Cow<'a, To>, ComposeMark<MarkConvertToRef, MarkCow>> for Field<T>
    where
        T: ConvertTo<'a, &'a To>,
        To: ToOwned + ?Sized,
    {
        fn filled(&'a self) -> Cow<'a, To> {
            let value = self.value_ref();
            Cow::Borrowed(value.convert_to())
        }

        fn changed(&'a self) -> Option<Cow<'a, To>> {
            let value = self.changed_ref();
            value.map(|v| Cow::Borrowed(v.convert_to()))
        }
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
