use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

struct Entry<T> {
    value: Arc<T>,
    expires_at: Instant,
}

/// A small TTL cache for one typed resource. Holding the mutex across the
/// loader await also de-duplicates concurrent requests, matching the Node
/// implementation.
pub struct CacheCell<T> {
    ttl: Duration,
    inner: Mutex<Option<Entry<T>>>,
}

impl<T> CacheCell<T> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            inner: Mutex::new(None),
        }
    }

    pub async fn get_or_try_init<E, F, Fut>(&self, loader: F) -> Result<Arc<T>, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let mut guard = self.inner.lock().await;
        if let Some(entry) = guard.as_ref()
            && entry.expires_at > Instant::now()
        {
            return Ok(Arc::clone(&entry.value));
        }

        let value = Arc::new(loader().await?);
        *guard = Some(Entry {
            value: Arc::clone(&value),
            expires_at: Instant::now() + self.ttl,
        });
        Ok(value)
    }
}

/// TTL map used for journey plans keyed by origin/destination/limit.
pub struct TtlMap {
    ttl: Duration,
    inner: Mutex<HashMap<String, (Instant, serde_json::Value)>>,
}

impl TtlMap {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get_or_try_init<E, F, Fut>(
        &self,
        key: String,
        loader: F,
    ) -> Result<serde_json::Value, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<serde_json::Value, E>>,
    {
        let mut guard = self.inner.lock().await;
        if let Some((expires_at, value)) = guard.get(&key)
            && *expires_at > Instant::now()
        {
            return Ok(value.clone());
        }

        let value = loader().await?;
        guard.insert(key, (Instant::now() + self.ttl, value.clone()));
        Ok(value)
    }
}
