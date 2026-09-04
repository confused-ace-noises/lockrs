use derive_more::{Deref, DerefMut, Eq};


#[derive(Debug, PartialEq, Eq, PartialOrd, Deref, DerefMut)]

/// a handy way to refer to a Wayland global, that tracks its actual global and
/// the name of said global.
pub struct Global<T> {
    #[deref]
    #[deref_mut]
    /// The actual global type
    pub global: T,
    
    /// The name of the global type
    pub name: u32,
}

impl<T> Global<T> {
    /// Create a new [`Global`] from its typesystem representation and its name
    pub fn new(global: T, name: u32) -> Self {
        Self { global, name }
    }
}