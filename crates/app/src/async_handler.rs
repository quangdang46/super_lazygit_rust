use std::sync::{Arc, Mutex};

struct State {
    current_id: i32,
    last_id: i32,
}

pub struct AsyncHandler {
    state: Arc<Mutex<State>>,
    on_reject: Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>,
    on_worker: Box<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>,
}

impl AsyncHandler {
    pub fn new<F>(on_worker: F) -> Self
    where
        F: Fn(Box<dyn FnOnce() + Send>) + Send + Sync + 'static,
    {
        Self {
            state: Arc::new(Mutex::new(State {
                current_id: 0,
                last_id: 0,
            })),
            on_reject: Arc::new(Mutex::new(None)),
            on_worker: Box::new(on_worker),
        }
    }

    pub fn on_reject<F>(&self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let mut reject = self.on_reject.lock().unwrap();
        *reject = Some(Box::new(f));
    }

    pub fn do_now<R, F>(&self, f: F)
    where
        R: FnOnce() + 'static,
        F: FnOnce() -> R + Send + 'static,
    {
        let (id, state, on_reject) = {
            let mut s = self.state.lock().unwrap();
            s.current_id += 1;
            let id = s.current_id;
            (id, Arc::clone(&self.state), Arc::clone(&self.on_reject))
        };

        let work = f;

        (self.on_worker)(Box::new(move || {
            let result = work();
            let should_reject = {
                let mut s = state.lock().unwrap();
                if id < s.last_id {
                    if let Some(ref reject) = *on_reject.lock().unwrap() {
                        reject();
                    }
                    return;
                }
                s.last_id = id;
                false
            };

            if !should_reject {
                result();
            }
        }));
    }
}

impl Clone for AsyncHandler {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            on_reject: Arc::clone(&self.on_reject),
            on_worker: Box::new(|_| {}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_async_handler_takes_latest() {
        let result = Arc::new(AtomicUsize::new(0));
        let result_clone = result.clone();
        let barrier = Arc::new(std::sync::Barrier::new(2));

        let handler = AsyncHandler::new(move |work: Box<dyn FnOnce() + Send>| {
            std::thread::spawn(move || {
                work();
            });
        });

        let barrier_clone = barrier.clone();
        handler.on_reject(move || {
            barrier_clone.wait();
        });

        let barrier_clone2 = barrier.clone();
        handler.do_now(move || {
            barrier_clone2.wait();
            move || {
                result_clone.store(1, Ordering::SeqCst);
            }
        });

        let result_clone2 = result.clone();
        handler.do_now(move || {
            move || {
                result_clone2.store(2, Ordering::SeqCst);
            }
        });

        barrier.wait();

        std::thread::sleep(std::time::Duration::from_millis(100));

        assert_eq!(result.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_async_handler_basic() {
        let counter = Arc::new(AtomicUsize::new(0));

        let handler = AsyncHandler::new(move |work: Box<dyn FnOnce() + Send>| {
            std::thread::spawn(move || {
                work();
            });
        });

        let counter_clone = counter.clone();
        handler.do_now(move || {
            move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }
        });

        std::thread::sleep(std::time::Duration::from_millis(50));

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
