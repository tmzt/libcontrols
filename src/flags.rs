//! Per-node flags: what a node declares about itself, and what the router
//! knows about it.

use std::ops::{BitAnd, BitOr, BitOrAssign, Not};

macro_rules! bit_set {
    ($(#[$meta:meta])* $name:ident { $($(#[$cmeta:meta])* $konst:ident = $bit:expr;)* }) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Default, PartialEq, Eq, Hash)]
        pub struct $name(u16);

        impl $name {
            /// No flag set.
            pub const NONE: Self = Self(0);
            $($(#[$cmeta])* pub const $konst: Self = Self(1 << $bit);)*

            /// Every flag of `other` is set in `self`.
            #[must_use]
            pub const fn contains(self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }

            /// Any flag of `other` is set in `self`.
            #[must_use]
            pub const fn intersects(self, other: Self) -> bool {
                self.0 & other.0 != 0
            }

            /// The flags of both.
            #[must_use]
            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }

            /// The flags of `self` that are not in `other`.
            #[must_use]
            pub const fn difference(self, other: Self) -> Self {
                Self(self.0 & !other.0)
            }

            /// `self` with the flags of `other` set or cleared.
            #[must_use]
            pub const fn with(self, other: Self, on: bool) -> Self {
                if on { self.union(other) } else { self.difference(other) }
            }

            /// No flag set.
            #[must_use]
            pub const fn is_empty(self) -> bool {
                self.0 == 0
            }
        }

        impl BitOr for $name {
            type Output = Self;
            fn bitor(self, rhs: Self) -> Self {
                self.union(rhs)
            }
        }

        impl BitOrAssign for $name {
            fn bitor_assign(&mut self, rhs: Self) {
                *self = self.union(rhs);
            }
        }

        impl BitAnd for $name {
            type Output = Self;
            fn bitand(self, rhs: Self) -> Self {
                Self(self.0 & rhs.0)
            }
        }

        impl Not for $name {
            type Output = Self;
            fn not(self) -> Self {
                Self(!self.0)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut set = f.debug_set();
                $(if self.contains(Self::$konst) {
                    set.entry(&stringify!($konst));
                })*
                set.finish()
            }
        }
    };
}

bit_set! {
    /// What a node declares about itself. Set from the control's
    /// `default_flags` when it is added; changed only through
    /// `Router::set_flags`, so the router always sees the change.
    NodeFlags {
        /// A Tab stop, and focused by a press on it.
        FOCUSABLE = 0;
        /// Takes no input and is no Tab stop, with its subtree; still drawn
        /// and still hit, so it blocks what lies beneath.
        DISABLED = 1;
        /// Not drawn, not hit and no Tab stop, with its subtree.
        HIDDEN = 2;
        /// On a nested router's node: its focus stops join the parent's Tab
        /// order instead of the router being one stop.
        TAB_THROUGH = 3;
    }
}

bit_set! {
    /// What the router knows about a node right now. Written only by the
    /// router; read by controls (in `DrawContext` and `EventContext`) and by
    /// the app.
    NodeState {
        /// This node holds the focus.
        FOCUSED = 0;
        /// A descendant holds the focus.
        FOCUS_WITHIN = 1;
        /// This node is the innermost one under the pointer.
        HOVERED = 2;
        /// A descendant is under the pointer.
        HOVER_WITHIN = 3;
        /// This node holds the press grab.
        GRABBED = 4;
    }
}
