use std::{
    fmt::format,
    marker::Send,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

use crate::IgnorePoisoned;

type Task = Box<dyn FnOnce() + Send + 'static>;

/// Panic resistant task context.
struct BusyGuard {
    busy_counter: Arc<AtomicUsize>,
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        let _ = self.busy_counter.fetch_sub(1, Ordering::Relaxed);
    }
}

impl BusyGuard {
    fn execute(self, task: Task) {
        let _ = self.busy_counter.fetch_add(1, Ordering::Relaxed);
        task(); // Can panic.
    }
}

struct WorkerCounter {
    id_counter: AtomicUsize,
    busy_counter: Arc<AtomicUsize>,
}

impl WorkerCounter {
    fn new() -> Self {
        Self {
            id_counter: AtomicUsize::new(0),
            busy_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn next_id(&self) -> usize {
        self.id_counter.fetch_add(1, Ordering::Relaxed)
    }

    fn busy(&self) -> BusyGuard {
        BusyGuard {
            busy_counter: Arc::clone(&self.busy_counter),
        }
    }
}

struct Worker {
    id: usize,
    join_handle: JoinHandle<()>,
}

impl Worker {
    fn new(
        id: usize,
        receiver: Arc<Mutex<Receiver<Task>>>,
        worker_counter: Arc<WorkerCounter>,
    ) -> Self {
        let join_handle = thread::Builder::new()
            .name(format!("Worker-{id}")) // TODO: use `thread::current().name()` for logging.
            .spawn(move || {
                println!("[ThreadPool] Worker #{id} waiting for tasks.");

                loop {
                    let task = match receiver.lock().ignore_poisoned().recv() {
                        Ok(task) => task,
                        Err(e) => {
                            println!("[ThreadPool] Worker #{id} shutting down: {e}.");
                            break;
                        }
                    };

                    println!("[ThreadPool] Worker #{id} got a task.");
                    worker_counter.busy().execute(task);
                    println!("[ThreadPool] Worker #{id} finished a task.");
                }
            })
            .unwrap(); // TODO: handle.

        Self { id, join_handle }
    }
}

pub struct ThreadPool {
    worker_counter: Arc<WorkerCounter>,
    sender: Sender<Task>,
    receiver: Arc<Mutex<Receiver<Task>>>,
    max_workers: usize,
    max_idle_workers: usize,
    workers: Vec<Worker>,
}

impl ThreadPool {
    fn new(builder: ThreadPoolBuilder) -> Self {
        let workers = Vec::with_capacity(builder.max_workers);

        let (sender, receiver) = mpsc::channel::<Task>();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut thread_pool = ThreadPool {
            worker_counter: Arc::new(WorkerCounter::new()),
            sender,
            receiver,
            max_workers: builder.max_workers,
            max_idle_workers: builder.max_idle_workers,
            workers,
        };

        for _ in 0..thread_pool.max_idle_workers {
            thread_pool.spawn_worker();
        }

        thread_pool
    }

    fn spawn_worker(&mut self) {
        assert!(
            self.workers.len() < self.max_workers,
            "Maximum number of workers exceeded!"
        );

        let id = self.worker_counter.next_id();
        let worker = Worker::new(
            id,
            Arc::clone(&self.receiver),
            Arc::clone(&self.worker_counter),
        );
        self.workers.push(worker);
    }

    pub fn builder() -> ThreadPoolBuilder {
        ThreadPoolBuilder {
            max_workers: 8,
            max_idle_workers: 1,
        }
    }
}

pub struct ThreadPoolBuilder {
    max_workers: usize,
    max_idle_workers: usize,
}

impl ThreadPoolBuilder {
    pub fn build(self) -> ThreadPool {
        assert!(
            self.max_idle_workers <= self.max_workers,
            "Number of idle threads cannot be greater than number of maximum threads."
        );
        ThreadPool::new(self)
    }

    pub fn max_threads(mut self, val: usize) -> Self {
        assert!(val > 0, "Maximum threads cannot be set to 0.");
        self.max_workers = val;
        self
    }

    pub fn idle_threads(mut self, val: usize) -> Self {
        self.max_idle_workers = val;
        self
    }
}

impl ThreadPool {
    pub fn execute<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let task = Box::new(f);
        self.sender.send(task).unwrap();

        let total_workers = self.workers.len();
        let busy_workers = self.worker_counter.busy_counter.load(Ordering::Relaxed);
        let idle_workers = total_workers - busy_workers;
        let idle_workers_to_spawn = self.max_idle_workers.saturating_sub(idle_workers);
        // If we are at 14 (out of 16 max) workers with 1 of them idle (out of 4), we have to spawn 3 more, but it will exceed max worker limit.
        let idle_workers_to_spawn = idle_workers_to_spawn.min(
            self.max_workers
                .saturating_sub(total_workers + idle_workers),
        );

        // Spawn extra idle workers if necessary.
        if idle_workers_to_spawn > 0 {
            for _ in 0..self.max_idle_workers {
                self.spawn_worker();
            }
        }

        // Kill exceeding workers if necessary.
        if idle_workers > 2 * self.max_idle_workers {
            let workers_to_kill =
                (total_workers - busy_workers).saturating_sub(self.max_idle_workers);
            println!("[ThreadPool] Sending {workers_to_kill} suicide requests.");
            for _ in 0..workers_to_kill {
                self.sender
                    .send(Box::new(|| {
                        panic!(
                            "Exceeding threads cleanup: {:}.",
                            thread::current().name().unwrap_or("unknown-thread")
                        )
                    })) // TODO: Task(Task)/Abort enum.
                    .unwrap();
            }
        }
    }
}
