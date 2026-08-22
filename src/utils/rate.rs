//! Shared token-bucket rate limiter for `--max-speed`.
//!
//! Implementation: a reservation slot. Each `acquire(n)` advances a global
//! `earliest-available` timestamp by `n / rate`; if that timestamp lies in the
//! future the caller sleeps until then. This paces the *aggregate* stream
//! exactly at `rate` bytes/second while allowing many concurrent workers to
//! reserve slices fairly (FIFO via the slot mutex).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use tokio::time::sleep;

pub struct RateLimiter {
    rate: u64,
    /// Earliest Instant at which the next token can be granted.
    slot: Mutex<Instant>,
}

impl RateLimiter {
    /// `rate` must be > 0.
    pub fn new(rate: u64) -> Result<Self> {
        if rate == 0 {
            bail!("rate limit must be greater than zero");
        }
        Ok(RateLimiter {
            rate,
            slot: Mutex::new(Instant::now()),
        })
    }

    /// An unthrottled limiter (used when `--max-speed` is absent).
    pub fn unlimited() -> Self {
        RateLimiter {
            rate: u64::MAX / 4,
            slot: Mutex::new(Instant::now()),
        }
    }

    /// Reserve `n` bytes: returns how long the caller must wait before the
    /// reservation becomes usable. Reservations are serialized so the
    /// aggregate transfer never exceeds `rate`.
    fn reserve(&self, n: u64) -> Duration {
        let mut slot = match self.slot.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let now = Instant::now();
        let need = if self.rate == u64::MAX / 4 {
            Duration::ZERO
        } else {
            let nanos = (n as u128)
                .saturating_mul(1_000_000_000)
                .checked_div(self.rate as u128)
                .unwrap_or(u128::MAX) as u64;
            Duration::from_nanos(nanos)
        };
        let start = (*slot).max(now);
        let wait = start
            .saturating_duration_since(now)
            .checked_add(need)
            .unwrap_or(Duration::from_secs(3600));
        *slot = start
            .checked_add(need)
            .unwrap_or(start + Duration::from_secs(3600));
        wait
    }

    /// Wait until `n` bytes may be transferred.
    pub async fn acquire(&self, n: usize) {
        let n = n as u64;
        if n == 0 {
            return;
        }
        let wait = self.reserve(n);
        if !wait.is_zero() {
            sleep(wait).await;
        }
    }

    /// Optimistic, non-reserving check: does the bucket currently have budget
    /// for `n` bytes? (Used by tests and the UI.)
    pub fn try_acquire(&self, n: usize) -> usize {
        let slot = match self.slot.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        if *slot <= Instant::now() {
            n
        } else {
            0
        }
    }

    pub fn rate(&self) -> u64 {
        self.rate
    }

    /// Convenience: an `Arc<RateLimiter>` builder.
    pub fn shared(self) -> std::sync::Arc<Self> {
        std::sync::Arc::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn unlimited_never_blocks() {
        let limiter = RateLimiter::unlimited().shared();
        let t0 = Instant::now();
        for n in [1024usize, 1 << 20, 1 << 20] {
            limiter.acquire(n).await;
        }
        assert!(t0.elapsed() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn limited_waits_for_tokens() {
        let limiter = Arc::new(RateLimiter::new(100_000).unwrap()); // 100 KiB/s
        let t0 = Instant::now();
        timeout(Duration::from_secs(6), async {
            limiter.acquire(200_000).await;
        })
        .await
        .expect("acquire must not hang");
        assert!(t0.elapsed() >= Duration::from_millis(1500));
        assert!(t0.elapsed() < Duration::from_millis(5000));
    }

    #[tokio::test]
    async fn two_workers_share_rate() {
        let limiter = Arc::new(RateLimiter::new(100_000).unwrap());
        let t0 = Instant::now();
        let a = {
            let l = limiter.clone();
            tokio::spawn(async move { l.acquire(100_000).await })
        };
        let b = {
            let l = limiter.clone();
            tokio::spawn(async move { l.acquire(100_000).await })
        };
        let _ = timeout(Duration::from_secs(6), async { a.await.unwrap(); b.await.unwrap(); })
            .await
            .expect("combined reservations must not deadlock");
        // 200 KiB at 100 KiB/s ≈ 2s, never more than ~3s.
        assert!(t0.elapsed() >= Duration::from_millis(1500));
    }

    #[test]
    fn optimistic_check() {
        let limiter = RateLimiter::new(1_000_000).unwrap();
        assert_eq!(limiter.try_acquire(10), 10);
    }
}
