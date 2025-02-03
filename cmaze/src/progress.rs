use std::sync::{Arc, Mutex, MutexGuard, RwLock};

#[derive(Clone, Debug)]
pub struct Flag(Arc<RwLock<bool>>);

impl Flag {
    pub fn new() -> Self {
        Flag(Arc::new(RwLock::new(false)))
    }

    pub fn stop(&self) {
        *self.0.write().unwrap() = true;
    }

    pub fn is_stopped(&self) -> bool {
        *self.0.read().unwrap()
    }
}

impl Default for Flag {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct ProgressHandle {
    progress: Arc<Mutex<Progress>>,
    children: Arc<Mutex<Vec<ProgressHandle>>>,
    flag: Flag,
}

impl ProgressHandle {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            progress: Arc::new(Mutex::new(Progress::new_empty())),
            children: Arc::new(Mutex::new(Vec::new())),
            flag: Flag::new(),
        }
    }

    pub fn split(&self) -> Self {
        let mut child = Self::new();
        child.flag = self.flag.clone();
        self.children.lock().unwrap().push(child.clone());
        child
    }

    pub fn lock(&self) -> MutexGuard<Progress> {
        self.progress.lock().unwrap()
    }

    pub fn progress(&self) -> Progress {
        let own = *self.lock();
        self.children
            .lock()
            .unwrap()
            .iter()
            .fold(own, |prog, child| prog.combine(&child.progress()))
    }

    pub fn stop(&self) {
        self.flag.stop();
    }

    pub fn is_stopped(&self) -> bool {
        self.flag.is_stopped()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Progress {
    pub done: usize,
    pub from: usize,
    pub is_done: bool,
}

impl Progress {
    pub fn new(done: usize, from: usize) -> Self {
        Self {
            done,
            from,
            is_done: false,
        }
    }

    pub fn new_empty() -> Self {
        Self::new(0, 0)
    }

    pub fn percent(&self) -> f32 {
        self.done as f32 / self.from as f32
    }

    pub fn finish(&mut self) {
        self.done = self.from;
        self.is_done = true;
    }

    pub fn combine(&self, other: &Self) -> Self {
        Self {
            done: self.done + other.done,
            from: self.from + other.from,
            is_done: self.is_done && other.is_done,
        }
    }
}
