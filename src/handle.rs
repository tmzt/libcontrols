//! Typed handle and value inspection traits for UI controls.

/// Trait implemented by controls that contain or compute a typed value of type `T`.
pub trait ReadValue<T> {
    /// Read the typed value from the control.
    fn read_value(&self) -> T;
}

/// A borrowed handle to a UI control allowing read-only access to its underlying value.
#[derive(Copy, Clone, Debug)]
pub struct ControlHandle<'a, C> {
    control: &'a C,
}

impl<'a, C> ControlHandle<'a, C> {
    /// Create a new control handle wrapping `control`.
    #[inline]
    pub const fn new(control: &'a C) -> Self {
        Self { control }
    }

    /// Read the typed value `T` from the underlying control through this handle.
    #[inline]
    pub fn read<T>(&self) -> T
    where
        C: ReadValue<T>,
    {
        self.control.read_value()
    }

    /// Access the underlying control reference.
    #[inline]
    pub const fn control(&self) -> &'a C {
        self.control
    }
}
