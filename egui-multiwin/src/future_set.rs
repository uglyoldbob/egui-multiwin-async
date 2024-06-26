//! Contains code for a hashset of futures that can be awaited

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
};

/// A set of futures, that finishes when any of the futures finishes
pub struct FuturesHashSetFirst<T> {
    i: Arc<Mutex<FuturesHashSetInternal<T>>>,
}

impl<T> Clone for FuturesHashSetFirst<T> {
    fn clone(&self) -> Self {
        Self { i: self.i.clone() }
    }
}

impl<T> FuturesHashSetFirst<T> {
    /// Construct a new self
    pub fn new() -> Self {
        Self {
            i: Arc::new(Mutex::new(FuturesHashSetInternal::new())),
        }
    }

    /// Get a reference to the inside
    pub fn get(&self) -> MutexGuard<'_, FuturesHashSetInternal<T>> {
        self.i.lock().unwrap()
    }
}

impl<T> std::future::Future for FuturesHashSetFirst<T> {
    type Output = T;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut s = self.i.lock().unwrap();
        if s.futures.is_empty() {
            return std::task::Poll::Pending;
        }
        for f in s.futures.values_mut() {
            if let std::task::Poll::Ready(ret) = f.as_mut().poll(cx) {
                return std::task::Poll::Ready(ret);
            }
        }
        return std::task::Poll::Pending;
    }
}

/// A set of futures
pub struct FuturesHashSetInternal<T> {
    futures: std::collections::HashMap<u32, Pin<Box<dyn Future<Output = T>>>>,
    gathered_outs: Vec<T>,
    last_index: u32,
}

/// A set of futures, that finishes when all of the futures finishes
pub struct FuturesHashSetAll<T> {
    i: Arc<Mutex<FuturesHashSetInternal<T>>>,
}

impl<T> Clone for FuturesHashSetAll<T> {
    fn clone(&self) -> Self {
        Self { i: self.i.clone() }
    }
}

impl<T> FuturesHashSetAll<T> {
    /// Construct a new self
    pub fn new() -> Self {
        Self {
            i: Arc::new(Mutex::new(FuturesHashSetInternal::new())),
        }
    }

    /// Get a reference to the inside
    pub fn get(&self) -> MutexGuard<'_, FuturesHashSetInternal<T>> {
        self.i.lock().unwrap()
    }
}

impl<T> FuturesHashSetInternal<T> {
    /// Construct a new self
    pub fn new() -> Self {
        Self {
            futures: std::collections::HashMap::new(),
            gathered_outs: Vec::new(),
            last_index: 0,
        }
    }

    /// Add a future to the list, returning an identifier that can be used to remove the future later
    pub fn add_future<F: Future<Output = T> + 'static>(&mut self, elem: F) -> u32 {
        let mut e = self.last_index + 1;
        loop {
            if !self.futures.contains_key(&e) {
                break;
            }
            e += 1;
        }
        self.futures.insert(e, Box::pin(elem));
        e
    }

    /// Remove a future previously added
    pub fn remove_future(&mut self, index: u32) {
        self.futures.remove(&index);
    }
}

impl<T: Clone> std::future::Future for FuturesHashSetAll<T> {
    type Output = Vec<T>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut s = self.i.lock().unwrap();
        let mut remove_me = Vec::new();
        let mut new_rets = Vec::new();
        for (i, f) in s.futures.iter_mut() {
            if let std::task::Poll::Ready(ret) = f.as_mut().poll(cx) {
                new_rets.push(ret);
                remove_me.push(i.to_owned());
            }
        }
        s.gathered_outs.append(&mut new_rets);
        for i in remove_me {
            s.futures.remove(&i);
        }
        if s.futures.is_empty() {
            return std::task::Poll::Ready(s.gathered_outs.clone());
        }
        return std::task::Poll::Pending;
    }
}

/// A set of futures, returning a stream of T
pub struct FuturesHashSet<T> {
    i: Arc<Mutex<FuturesHashSetInternal<T>>>,
}

impl<T> Clone for FuturesHashSet<T> {
    fn clone(&self) -> Self {
        Self { i: self.i.clone() }
    }
}

impl<T> FuturesHashSet<T> {
    /// Construct a new self
    pub fn new() -> Self {
        Self {
            i: Arc::new(Mutex::new(FuturesHashSetInternal::new())),
        }
    }

    /// Get a reference to the inside
    pub fn get(&self) -> MutexGuard<'_, FuturesHashSetInternal<T>> {
        self.i.lock().unwrap()
    }
}

impl<T: Clone> futures_lite::Stream for FuturesHashSet<T> {
    type Item = T;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut s = self.i.lock().unwrap();
        let mut remove_me = Vec::new();
        let mut next_val = None;
        for (i, f) in s.futures.iter_mut() {
            if let std::task::Poll::Ready(ret) = f.as_mut().poll(cx) {
                remove_me.push(i.to_owned());
                next_val = Some(ret);
                break;
            }
        }
        for i in remove_me {
            s.futures.remove(&i);
        }
        if next_val.is_some() {
            std::task::Poll::Ready(next_val)
        } else {
            std::task::Poll::Pending
        }
    }
}
