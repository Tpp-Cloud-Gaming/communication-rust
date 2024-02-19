use std::sync::Arc;
use tokio::sync::{AcquireError, Mutex, Semaphore, SemaphorePermit, TryAcquireError};

#[derive(Clone)]
pub struct Shutdown {
    error_notifier: Arc<Semaphore>,
    shutdown_notifier: Arc<Semaphore>,
    counter: Arc<Mutex<u32>>,
}

impl Shutdown {
    pub fn new() -> Self {
        Self {
            error_notifier: Arc::new(Semaphore::new(0)),
            shutdown_notifier: Arc::new(Semaphore::new(0)),
            counter: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn wait_for_shutdown(&self) -> Result<SemaphorePermit<'_>, AcquireError> {
        self.shutdown_notifier.acquire().await
    }

    pub async fn wait_for_error(&self) -> Result<SemaphorePermit<'_>, AcquireError> {
        let r = self.error_notifier.acquire().await;
        let mut counter = self.counter.lock().await;
        *counter -= 1;
        if *counter == 0 {
            self.shutdown_notifier.add_permits(1);
        }
        return r;
    }

    pub async fn add_task(&self) {
        let mut counter = self.counter.lock().await;
        *counter += 1;
    }

    // Task who triggered the shutdown will call this method
    pub async fn notify_error(&self, main_task: bool) {
        let mut counter = self.counter.lock().await;

        if !main_task {
            *counter -= 1;
        }
        if *counter == 0 {
            self.shutdown_notifier.add_permits(1);
        }
        self.error_notifier.add_permits(*counter as usize);
    }

    pub async fn check_for_error(&self) -> bool {
        match self.error_notifier.try_acquire() {
            Ok(_) => {
                let mut counter = self.counter.lock().await;
                *counter -= 1;
                if *counter == 0 {
                    self.shutdown_notifier.add_permits(1);
                }
                return true;
            }
            Err(TryAcquireError::Closed) => {
                self.notify_error(false).await;
                return true;
            }
            Err(TryAcquireError::NoPermits) => {
                return false;
            }
        };
    }

    pub fn shutdown(&self) {
        self.shutdown_notifier.close();
        self.error_notifier.close();
    }
}
