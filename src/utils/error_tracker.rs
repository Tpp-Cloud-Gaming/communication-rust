/// A struct for tracking errors and iterations, and raising an error when a threshold is reached.
pub struct ErrorTracker {
    iterations_counter: u32,
    errors_counter: u32,
    threshold: u32,
    reset_limit: u32,
}

impl ErrorTracker {
    /// Constructs a new ErrorTracker instance with the specified threshold and reset limit.
    ///
    /// # Arguments
    ///
    /// * `threshold` - The error threshold. When the number of errors reaches this threshold, an error will be raised.
    /// * `reset_limit` - The iteration count at which the error and iteration counters are reset to zero.
    ///
    /// # Returns
    ///
    /// A new ErrorTracker instance.
    pub fn new(threshold: u32, reset_limit: u32) -> Self {
        Self {
            iterations_counter: 0,
            errors_counter: 0,
            threshold,
            reset_limit,
        }
    }
    /// Increments the error counter and iteration counter, and checks if the error threshold has been reached.
    ///
    /// # Returns
    ///
    /// `true` if the error threshold has been reached, `false` otherwise.
    pub fn increment_with_error(&mut self) -> bool {
        self.errors_counter += 1;
        self.iterations_counter += 1;

        self.errors_counter >= self.threshold
    }

    /// Increments the iteration counter, and resets the counters if the reset limit is reached.
    pub fn increment(&mut self) {
        self.iterations_counter += 1;

        if self.iterations_counter >= self.reset_limit {
            self.reset();
        }
    }
    /// Resets the iteration and error counters to zero.
    fn reset(&mut self) {
        self.iterations_counter = 0;
        self.errors_counter = 0;
    }
}
