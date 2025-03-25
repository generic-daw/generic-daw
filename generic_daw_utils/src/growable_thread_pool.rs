use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicUsize, Ordering::AcqRel},
    mpsc::{self, Receiver, Sender},
};

struct GrowableThreadPool {
    sender: Sender<Box<dyn FnOnce() + Send + 'static>>,
    receiver: Mutex<Receiver<Box<dyn FnOnce() + Send + 'static>>>,
    waiting_threads: AtomicUsize,
}

impl GrowableThreadPool {
    fn new() -> Self {
        Self::spawn_thread();

        let (sender, receiver) = mpsc::channel();

        Self {
            sender,
            receiver: Mutex::new(receiver),
            waiting_threads: AtomicUsize::new(0),
        }
    }

    fn spawn_thread() {
        std::thread::spawn(|| {
            loop {
                THREAD_POOL.waiting_threads.fetch_add(1, AcqRel);

                let Ok(Ok(f)) = THREAD_POOL.receiver.lock().map(|r| r.recv()) else {
                    return;
                };

                if THREAD_POOL.waiting_threads.fetch_sub(1, AcqRel) == 1 {
                    Self::spawn_thread();
                }

                f();
            }
        });
    }
}

static THREAD_POOL: LazyLock<GrowableThreadPool> = LazyLock::new(GrowableThreadPool::new);

pub fn spawn(f: impl FnOnce() + Send + 'static) {
    THREAD_POOL.sender.send(Box::new(f)).unwrap();
}
