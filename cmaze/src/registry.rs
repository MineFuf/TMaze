use std::{hash::Hash, sync::Arc};

use hashbrown::{Equivalent, HashMap};

pub struct Registry<T: ?Sized, K = String> {
    items: HashMap<K, Arc<T>>,
    default: Option<Arc<T>>,
}

impl<T: ?Sized, K> Registry<T, K> {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            default: None,
        }
    }

    pub fn get_default(&self) -> Option<Arc<T>> {
        self.default.clone()
    }
}

impl<T: ?Sized, K> Default for Registry<T, K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized, K> Registry<T, K>
where
    K: Hash + Eq,
{
    pub fn with_default(default: Arc<T>, keyed: impl Into<K>) -> Self {
        Self {
            items: [(keyed.into(), default.clone())].into_iter().collect(),
            default: Some(default),
        }
    }

    pub fn register(&mut self, key: impl Into<K>, item: Arc<T>) {
        self.items.insert(key.into(), item);
    }

    pub fn get<Q>(&self, k: &Q) -> Option<Arc<T>>
    where
        Q: Hash + Equivalent<K>,
    {
        self.items.get(k).cloned()
    }

    pub fn is_registered(&self, k: &K) -> bool {
        self.items.contains_key(k)
    }
}
