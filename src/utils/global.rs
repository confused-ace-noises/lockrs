use derive_more::{Deref, DerefMut, Eq};


#[derive(Debug, PartialEq, Eq, PartialOrd, Deref, DerefMut)]
pub struct Global<T> {
    #[deref]
    #[deref_mut]
    pub global: T,
    
    pub name: u32,
}

impl<T> Global<T> {
    pub fn new(global: T, name: u32) -> Self {
        Self { global, name }
    }
}