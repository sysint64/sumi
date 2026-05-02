use std::ops::{Deref, DerefMut};

pub struct LazyGraphicsResource<T> {
    data: Option<T>,
}

impl<T> Default for LazyGraphicsResource<T> {
    fn default() -> Self {
        Self { data: None }
    }
}

impl<T> LazyGraphicsResource<T> {
    pub fn new() -> Self {
        Self { data: None }
    }

    pub fn initialized(&self) -> bool {
        self.data.is_some()
    }

    pub fn init(&mut self, data: T) {
        debug_assert!(
            self.data.is_none(),
            "Resrource already has been initialized"
        );
        self.data.replace(data);
    }
}

impl<T> Deref for LazyGraphicsResource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
            .as_ref()
            .expect("Resrource has not been initialized")
    }
}

impl<T> DerefMut for LazyGraphicsResource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
            .as_mut()
            .expect("Resrource has not been initialized")
    }
}

impl<T> AsRef<T> for LazyGraphicsResource<T> {
    fn as_ref(&self) -> &T {
        self.data
            .as_ref()
            .expect("Resrource has not been initialized")
    }
}

impl<T> AsMut<T> for LazyGraphicsResource<T> {
    fn as_mut(&mut self) -> &mut T {
        self.data
            .as_mut()
            .expect("Resrource has not been initialized")
    }
}
