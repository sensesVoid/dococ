use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Run a closure with a timeout. Returns Ok(result) or Err("timeout").
pub fn run_with_timeout<F, T>(f: F, timeout: Duration) -> Result<T, &'static str>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = f();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => Ok(result),
        Err(mpsc::RecvTimeoutError::Timeout) => Err("timeout"),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("thread panicked"),
    }
}

/// Inference timeout configuration.
#[derive(Debug, Clone, Copy)]
pub struct InferenceTimeout {
    pub load_ms: u64,
    pub infer_ms: u64,
}

impl Default for InferenceTimeout {
    fn default() -> Self {
        Self {
            load_ms: 30_000,
            infer_ms: 10_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_with_timeout_succeeds() {
        let result = run_with_timeout(|| 42, Duration::from_secs(1));
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn run_with_timeout_times_out() {
        let result = run_with_timeout(
            || {
                std::thread::sleep(Duration::from_secs(10));
                42
            },
            Duration::from_millis(50),
        );
        assert_eq!(result, Err("timeout"));
    }

    #[test]
    fn inference_timeout_default() {
        let t = InferenceTimeout::default();
        assert_eq!(t.load_ms, 30_000);
        assert_eq!(t.infer_ms, 10_000);
    }
}
