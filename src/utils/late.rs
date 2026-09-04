use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};


/// a thin wrapper around a [`MaybeUninit`], that simply tracks whether it
/// init'ed or not, and always dereferences to the init'ed value.
/// Dereferencing this in an uninit'ed state will cause UB.
/// 
/// # Example
/// ```
/// # use lockrs::utils::late::Late;
/// let mut late = Late::<String>::uninit();
/// // let inner = *late; // UB!
/// late.init(String::from("sorry im late"));
/// assert_eq!((*late).as_str(), "sorry im late");
/// ```
pub struct Late<T>{
    maybe_uninit: MaybeUninit<T>,
    has_init: bool
}

impl<T> Default for Late<T> {
    fn default() -> Self {
        Self::uninit()
    }
}

impl<T> Late<T> {
    /// create a new uninitialized [`Late`].
    pub const fn uninit() -> Self {
        Late {
            maybe_uninit: MaybeUninit::uninit(),
            has_init: false,
        }
    }

    /// initialized [`Late`] to a value. 
    /// If [`Late`] had already been initialized, the old value will be
    /// dropped and in its place the new value will be written.
    pub fn init(&mut self, val: T) {
        if self.is_init() {
            unsafe { self.maybe_uninit.assume_init_drop() };
            self.maybe_uninit.write(val);
        } else {
            self.maybe_uninit.write(val);
            self.has_init = true;
        }
    }

    /// checks whether [`Late::init`] has been called.
    pub fn is_init(&self) -> bool {
        self.has_init
    }  
}

impl<T> Deref for Late<T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: caller guarantees init
        unsafe { self.maybe_uninit.assume_init_ref() }
    }
}

impl<T> DerefMut for Late<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.maybe_uninit.assume_init_mut() }
    }
}