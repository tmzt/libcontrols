//! `Symbol` — interned names, compared and hashed as integers.

use std::collections::HashMap;
use std::fmt;
use std::sync::{OnceLock, RwLock};

/// An interned string. Equal symbols are the same name; comparing two is an
/// integer compare. Names are never freed: the UI vocabulary is small.
///
/// `Ord` is interning order, for use as a map key, not for display. `Symbol`
/// does not implement `Borrow<str>`: its hash is the integer's, which would
/// disagree with `str`'s in a `HashMap`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(u32);

struct Interner {
    by_name: HashMap<&'static str, u32>,
    names: Vec<&'static str>,
}

static INTERNER: OnceLock<RwLock<Interner>> = OnceLock::new();

fn interner() -> &'static RwLock<Interner> {
    INTERNER.get_or_init(|| {
        RwLock::new(Interner {
            by_name: HashMap::new(),
            names: Vec::new(),
        })
    })
}

impl Symbol {
    /// The symbol for `name`, interning it on first sight.
    pub fn intern(name: &str) -> Symbol {
        let lock = interner();
        if let Some(&id) = lock.read().unwrap().by_name.get(name) {
            return Symbol(id);
        }
        let mut table = lock.write().unwrap();
        // Another thread may have added it between the two locks.
        if let Some(&id) = table.by_name.get(name) {
            return Symbol(id);
        }
        let id = u32::try_from(table.names.len()).expect("fewer than 2^32 symbols");
        let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
        table.names.push(leaked);
        table.by_name.insert(leaked, id);
        Symbol(id)
    }

    /// The symbol for `name` if it has been interned, without interning it.
    pub fn lookup(name: &str) -> Option<Symbol> {
        interner()
            .read()
            .unwrap()
            .by_name
            .get(name)
            .map(|&id| Symbol(id))
    }

    /// The name. Lives as long as the process.
    pub fn as_str(self) -> &'static str {
        interner().read().unwrap().names[self.0 as usize]
    }

    /// The symbol's index in the interner: dense from 0, in order of first
    /// interning. Stable within a process only.
    pub const fn id(self) -> u32 {
        self.0
    }
}

impl From<&str> for Symbol {
    fn from(name: &str) -> Self {
        Symbol::intern(name)
    }
}

impl From<String> for Symbol {
    fn from(name: String) -> Self {
        Symbol::intern(&name)
    }
}

impl From<&String> for Symbol {
    fn from(name: &String) -> Self {
        Symbol::intern(name)
    }
}

impl std::ops::Deref for Symbol {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<str> for Symbol {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Symbol {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

/// The symbol for a string literal, interned once per call site.
#[macro_export]
macro_rules! sym {
    ($name:literal) => {{
        static SYM: ::std::sync::OnceLock<$crate::Symbol> = ::std::sync::OnceLock::new();
        *SYM.get_or_init(|| $crate::Symbol::intern($name))
    }};
}
