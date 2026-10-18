//! Cancellation, deadlines and progress reporting shared by every long operation.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use web_time::Instant;

use crate::error::{Error, ErrorKind, Result};

/// A cloneable flag that stops the operations it was given to. Cancelling is cooperative: decoders
/// and muxers poll it between macroblock rows, packets and frames and return an error of kind
/// [`crate::ErrorKind::Cancelled`]. A token stays cancelled; make a new one for the next operation.
///
/// `CancelToken` is `Send + Sync`: cancel from any thread.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token that is not cancelled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask every operation holding this token to stop.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether [`cancel`](Self::cancel) was called.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    #[cfg(feature = "decode")]
    pub(crate) fn flag(&self) -> &Arc<AtomicBool> {
        &self.0
    }
}

/// The phase an operation is in, reported with [`Progress`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Stage {
    /// Reading the container index.
    Indexing,
    /// Reading packets from the input.
    Reading,
    /// Decoding frames or audio.
    Decoding,
    /// Encoding frames or audio.
    Encoding,
    /// Writing the output container.
    Writing,
    /// Flushing encoders and finalising the output.
    Finishing,
}

/// A progress report: `done` of `total` units of the current [`Stage`] (packets for remuxing,
/// frames for encoding and decoding batches). `total` is `None` when it is not known up front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Progress {
    /// What the operation is doing.
    pub stage: Stage,
    /// Units finished so far.
    pub done: u64,
    /// Units in the whole operation, when known.
    pub total: Option<u64>,
}

impl Progress {
    /// Fraction finished in `0.0..=1.0`, when `total` is known and non-zero.
    pub fn fraction(&self) -> Option<f64> {
        let total = self.total.filter(|t| *t > 0)?;
        Some((self.done.min(total) as f64) / (total as f64))
    }
}

type ProgressFn = dyn Fn(&Progress) + Send + Sync;

/// Cancellation, an optional time budget and an optional progress callback, passed by reference
/// to the operations that can run long. `Control::default()` never cancels and never times out.
///
/// The type is `Clone + Send + Sync`; clones share the cancel token and the callback.
#[derive(Clone, Default)]
pub struct Control {
    cancel: CancelToken,
    deadline: Option<Instant>,
    progress: Option<Arc<ProgressFn>>,
}

impl std::fmt::Debug for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Control").field("cancel", &self.cancel).field("deadline", &self.deadline).field("progress", &self.progress.is_some()).finish()
    }
}

impl Control {
    /// No cancellation, no deadline, no callback.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stop when `token` is cancelled.
    pub fn with_cancel(mut self, token: CancelToken) -> Self {
        self.cancel = token;
        self
    }

    /// Stop with [`crate::ErrorKind::DeadlineExceeded`] once `timeout` has elapsed from now.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.deadline = Instant::now().checked_add(timeout);
        self
    }

    /// Stop with [`crate::ErrorKind::DeadlineExceeded`] at `deadline`.
    pub fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Call `f` after each unit of work. It runs on the calling thread, must be quick, and must
    /// not call back into the operation.
    pub fn with_progress(mut self, f: impl Fn(&Progress) + Send + Sync + 'static) -> Self {
        self.progress = Some(Arc::new(f));
        self
    }

    /// The cancel token of this control.
    pub fn cancel_token(&self) -> &CancelToken {
        &self.cancel
    }

    /// `Err` when the operation must stop: cancelled or past its deadline.
    pub fn check(&self) -> Result<()> {
        if self.cancel.is_cancelled() {
            return Err(Error::new(ErrorKind::Cancelled, "cancelled"));
        }
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(Error::new(ErrorKind::DeadlineExceeded, "deadline exceeded"));
        }
        Ok(())
    }

    #[cfg_attr(not(any(feature = "container", feature = "decode", feature = "encode")), allow(dead_code))]
    pub(crate) fn report(&self, stage: Stage, done: u64, total: Option<u64>) {
        if let Some(f) = &self.progress {
            f(&Progress { stage, done, total });
        }
    }

    /// Run `f` (a decode call that may take long) so that cancellation and the deadline reach into
    /// the decoders, and turn their "cancelled" error into the right kind.
    #[cfg(feature = "decode")]
    pub(crate) fn guard<R>(&self, f: impl FnOnce() -> Result<R>) -> Result<R> {
        self.check()?;
        let Some(deadline) = self.deadline else {
            return finish(self, filmcraft_media::cancel::with_cancel(self.cancel.flag(), f));
        };
        let internal = Arc::new(AtomicBool::new(false));
        let expired = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let watcher = watch(self.cancel.clone(), deadline, internal.clone(), expired.clone(), stop.clone());
        let r = filmcraft_media::cancel::with_cancel(&internal, f);
        stop.store(true, Ordering::SeqCst);
        if let Some(h) = watcher {
            let _ = h.join();
        }
        if expired.load(Ordering::SeqCst) && r.is_err() {
            return Err(Error::new(ErrorKind::DeadlineExceeded, "deadline exceeded"));
        }
        finish(self, r)
    }
}

#[cfg(feature = "decode")]
fn finish<R>(c: &Control, r: Result<R>) -> Result<R> {
    match r {
        Err(e) if e.kind() == ErrorKind::Cancelled && !c.cancel.is_cancelled() => Err(Error::new(ErrorKind::DeadlineExceeded, "deadline exceeded")),
        other => other,
    }
}

/// A thread that raises `internal` once the token is cancelled or the deadline passes. On wasm
/// there are no threads: the deadline is then only checked between calls.
#[cfg(all(feature = "decode", not(target_arch = "wasm32")))]
fn watch(
    token: CancelToken,
    deadline: Instant,
    internal: Arc<AtomicBool>,
    expired: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> Option<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("filmcraft-lib-watchdog".into())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                if token.is_cancelled() {
                    internal.store(true, Ordering::SeqCst);
                    return;
                }
                if Instant::now() >= deadline {
                    expired.store(true, Ordering::SeqCst);
                    internal.store(true, Ordering::SeqCst);
                    return;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        })
        .ok()
}

#[cfg(all(feature = "decode", target_arch = "wasm32"))]
fn watch(_: CancelToken, _: Instant, _: Arc<AtomicBool>, _: Arc<AtomicBool>, _: Arc<AtomicBool>) -> Option<std::thread::JoinHandle<()>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_token_is_shared_between_clones() {
        let t = CancelToken::new();
        let c = Control::new().with_cancel(t.clone());
        assert!(c.check().is_ok());
        t.cancel();
        assert_eq!(c.check().unwrap_err().kind(), ErrorKind::Cancelled);
    }

    #[test]
    fn deadline_in_the_past_stops_immediately() {
        let c = Control::new().with_timeout(Duration::ZERO);
        assert_eq!(c.check().unwrap_err().kind(), ErrorKind::DeadlineExceeded);
        assert!(c.check().unwrap_err().is_interrupted());
    }

    #[test]
    fn progress_fraction_handles_unknown_and_zero_totals() {
        let p = |done, total| Progress { stage: Stage::Reading, done, total };
        assert_eq!(p(5, None).fraction(), None);
        assert_eq!(p(5, Some(0)).fraction(), None);
        assert_eq!(p(5, Some(10)).fraction(), Some(0.5));
        assert_eq!(p(50, Some(10)).fraction(), Some(1.0));
    }

    #[test]
    fn progress_callback_runs() {
        use std::sync::atomic::AtomicU64;
        let n = Arc::new(AtomicU64::new(0));
        let n2 = n.clone();
        let c = Control::new().with_progress(move |p| n2.store(p.done, Ordering::SeqCst));
        c.report(Stage::Writing, 7, Some(9));
        assert_eq!(n.load(Ordering::SeqCst), 7);
    }

    #[test]
    fn control_is_send_sync() {
        fn assert_send_sync<T: Send + Sync + Clone + 'static>() {}
        assert_send_sync::<Control>();
        assert_send_sync::<CancelToken>();
    }
}
