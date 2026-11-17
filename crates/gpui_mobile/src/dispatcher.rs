use gpui::{PlatformDispatcher, Priority, RunnableVariant};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, ThreadId},
    time::{Duration, Instant},
};

pub(crate) struct Dispatcher {
    owner: ThreadId,
    main: Mutex<VecDeque<RunnableVariant>>,
    background: Mutex<Option<mpsc::Sender<RunnableVariant>>>,
    timers: Mutex<Option<mpsc::Sender<(Instant, RunnableVariant)>>>,
    stopped: AtomicBool,
}

impl Dispatcher {
    pub fn new() -> Arc<Self> {
        let (background, receiver) = mpsc::channel::<RunnableVariant>();
        let receiver = Arc::new(Mutex::new(receiver));
        for index in 0..2 {
            let receiver = receiver.clone();
            thread::Builder::new()
                .name(format!("gpui-mobile-{index}"))
                .spawn(move || {
                    loop {
                        let task = receiver.lock().unwrap().recv();
                        match task {
                            Ok(task) => {
                                task.run();
                            }
                            Err(_) => break,
                        }
                    }
                })
                .expect("mobile background worker");
        }
        let (timers, receiver) = mpsc::channel::<(Instant, RunnableVariant)>();
        let jobs = background.clone();
        thread::Builder::new()
            .name("gpui-mobile-timers".into())
            .spawn(move || {
                let mut pending: Vec<(Instant, RunnableVariant)> = Vec::new();
                loop {
                    pending.sort_by_key(|entry| entry.0);
                    let next = match pending.first() {
                        Some((due, _)) => {
                            receiver.recv_timeout(due.saturating_duration_since(Instant::now()))
                        }
                        None => receiver
                            .recv()
                            .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
                    };
                    match next {
                        Ok(job) => pending.push(job),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let (_, task) = pending.remove(0);
                            if jobs.send(task).is_err() {
                                break;
                            }
                        }
                    }
                }
            })
            .expect("mobile timer worker");
        Arc::new(Self {
            owner: thread::current().id(),
            main: Mutex::default(),
            background: Mutex::new(Some(background)),
            timers: Mutex::new(Some(timers)),
            stopped: AtomicBool::new(false),
        })
    }

    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
        let pending = std::mem::take(&mut *self.main.lock().unwrap());
        drop(pending);
        self.timers.lock().unwrap().take();
        self.background.lock().unwrap().take();
    }

    pub fn drain(&self) {
        assert!(
            self.is_main_thread(),
            "host calls must stay on their creating UI thread"
        );
        // A self-waking task must not starve UIKit/Choreographer and input delivery.
        for _ in 0..128 {
            let task = self.main.lock().unwrap().pop_front();
            let Some(task) = task else { break };
            task.run();
        }
    }
}

impl PlatformDispatcher for Dispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.owner
    }
    fn dispatch(&self, runnable: RunnableVariant, _: Priority) {
        if let Some(sender) = self.background.lock().unwrap().as_ref() {
            let _ = sender.send(runnable);
        }
    }
    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, _: Priority) {
        let mut main = self.main.lock().unwrap();
        if !self.stopped.load(Ordering::Acquire) {
            main.push_back(runnable);
        }
    }
    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        if let Some(sender) = self.timers.lock().unwrap().as_ref() {
            let _ = sender.send((Instant::now() + duration, runnable));
        }
    }
    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        thread::spawn(f);
    }
}

#[cfg(test)]
mod tests {
    use super::Dispatcher;
    use gpui::ForegroundExecutor;
    use std::{cell::Cell, rc::Rc, sync::Arc};

    #[test]
    fn shutdown_breaks_the_queued_task_executor_cycle() {
        let dispatcher = Dispatcher::new();
        let weak = Arc::downgrade(&dispatcher);
        let ran = Rc::new(Cell::new(false));
        let flag = ran.clone();
        let executor = ForegroundExecutor::new(dispatcher.clone());
        executor
            .spawn(async move {
                flag.set(true);
            })
            .detach();
        dispatcher.shutdown();
        drop(executor);
        drop(dispatcher);
        assert!(weak.upgrade().is_none());
        assert!(!ran.get());
    }
}
