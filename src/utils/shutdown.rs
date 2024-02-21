use std::sync::Arc;
use tokio::sync::{AcquireError, Mutex, Semaphore, SemaphorePermit, TryAcquireError};

#[derive(Clone)]
pub struct Shutdown {
    error_notifier: Arc<Semaphore>,
    shutdown_notifier: Arc<Semaphore>,
    counter: Arc<Mutex<u32>>,
    notifier_active: Arc<Mutex<bool>>,
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new()
    }
}

impl Shutdown {
    pub fn new() -> Self {
        Self {
            error_notifier: Arc::new(Semaphore::new(0)),
            shutdown_notifier: Arc::new(Semaphore::new(0)),
            counter: Arc::new(Mutex::new(0)),
            notifier_active: Arc::new(Mutex::new(false)),
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
        r
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
            _ = self.wait_for_shutdown().await;
        }
        if *counter == 0 {
            self.shutdown_notifier.add_permits(1);
        }
        let mut notifier_active = self.notifier_active.lock().await;
        if !(*notifier_active) {
            self.error_notifier.add_permits(*counter as usize);
            *notifier_active = true;
        }
    }

    pub async fn check_for_error(&self) -> bool {
        match self.error_notifier.try_acquire() {
            Ok(_) => {
                let mut counter = self.counter.lock().await;
                *counter -= 1;
                if *counter == 0 {
                    self.shutdown_notifier.add_permits(1);
                }
                true
            }
            Err(TryAcquireError::Closed) => {
                self.notify_error(false).await;
                true
            }
            Err(TryAcquireError::NoPermits) => false,
        }
    }

    pub fn shutdown(&self) {
        self.shutdown_notifier.close();
        self.error_notifier.close();
    }
}
