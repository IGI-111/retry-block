//! Different types of delay for retryable operations.

use std::time::Duration;

mod random;

pub use random::{jitter, jitter_rng, Range};

/// The sum of cumulative retry delays is bounded by some finite amount.
#[derive(Debug)]
pub struct Bounded<T> {
    inner: T,
    acc: Duration,
    max: Duration,
}

impl<T> Bounded<T>
where
    T: Iterator<Item = Duration>,
{
    pub fn new<U>(inner: U, max: Duration) -> Self
    where
        U: IntoIterator<Item = Duration, IntoIter = T>,
    {
        Self {
            inner: inner.into_iter(),
            max,
            acc: Default::default(),
        }
    }
}

impl<T> Iterator for Bounded<T>
where
    T: Iterator<Item = Duration>,
{
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        self.inner.next().filter(|next| {
            if let Some(acc) = self.acc.checked_add(*next) {
                self.acc = acc;
                self.acc <= self.max
            } else {
                false
            }
        })
    }
}

/// Each retry increases the delay since the last exponentially.
#[derive(Debug, Clone)]
pub struct Exponential {
    current: Duration,
    factor: f64,
}

impl Exponential {
    /// Creates a new `Exponential` using a random proportion of the given
    /// duration as the initial delay.
    pub fn new(duration: Duration) -> Self {
        Self::with_factor(duration, duration.as_millis() as f64)
    }

    /// Creates a new `Exponential` using a random proportion of the given
    /// duration as the initial delay and a variable multiplication factor.
    pub fn with_factor(base: Duration, factor: f64) -> Self {
        Self {
            current: jitter(base),
            factor,
        }
    }

    /// Creates a new `Exponential` using the given duration as the initial
    /// delay.
    pub fn exact(duration: Duration) -> Self {
        Self::exact_with_factor(duration, duration.as_millis() as f64)
    }

    /// Creates a new `Exponential` using the given duration as the initial
    /// delay and a variable multiplication factor.
    pub fn exact_with_factor(base: Duration, factor: f64) -> Self {
        Self {
            current: base,
            factor,
        }
    }

    /// Applies an upper bound of `max` to this exponential delay generator.
    pub fn bounded(self, max: Duration) -> Bounded<Self> {
        Bounded::new(self, max)
    }
}

impl Iterator for Exponential {
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        fn try_from_secs_f64(secs: f64) -> Option<Duration> {
            const NANOS_PER_SEC: u32 = 1_000_000_000;
            const MAX_NANOS_F64: f64 = ((u64::MAX as u128 + 1) * (NANOS_PER_SEC as u128)) as f64;
            let nanos = secs * (NANOS_PER_SEC as f64);
            if !nanos.is_finite() || nanos >= MAX_NANOS_F64 || nanos < 0.0 {
                None
            } else {
                Some(Duration::from_secs_f64(secs))
            }
        }

        let duration = self.current;

        let next_secs = self.current.as_secs_f64() * self.factor;
        self.current = try_from_secs_f64(next_secs).unwrap_or(self.current);

        Some(duration)
    }
}

impl From<Duration> for Exponential {
    fn from(duration: Duration) -> Self {
        Self::new(duration)
    }
}

#[test]
fn exponential_with_factor() {
    let mut iter = Exponential::exact_with_factor(Duration::from_secs(1), 2.0);
    assert_eq!(iter.next(), Some(Duration::from_secs(1)));
    assert_eq!(iter.next(), Some(Duration::from_secs(2)));
    assert_eq!(iter.next(), Some(Duration::from_secs(4)));
    assert_eq!(iter.next(), Some(Duration::from_secs(8)));
    assert_eq!(iter.next(), Some(Duration::from_secs(16)));
    assert_eq!(iter.next(), Some(Duration::from_secs(32)));
}

#[test]
fn exponential_overflow() {
    let mut iter = Exponential::exact(Duration::MAX);
    assert_eq!(iter.next(), Some(Duration::MAX));
    assert_eq!(iter.next(), Some(Duration::MAX));
}

#[test]
fn exponential_with_upper_bound() {
    let mut iter =
        Exponential::exact_with_factor(Duration::from_secs(1), 2.0).bounded(Duration::from_secs(4));
    assert_eq!(iter.next(), Some(Duration::from_secs(1)));
    assert_eq!(iter.next(), Some(Duration::from_secs(2)));
    // 1 + 2 + 4 > 5 => upper bound would be reached
    assert_eq!(iter.next(), None);
}

/// Each retry uses a delay which is the sum of the two previous delays.
///
/// Depending on the problem at hand, a fibonacci delay strategy might
/// perform better and lead to better throughput than the `Exponential`
/// strategy.
///
/// See ["A Performance Comparison of Different Backoff Algorithms under Different Rebroadcast Probabilities for MANETs."](http://www.comp.leeds.ac.uk/ukpew09/papers/12.pdf)
/// for more details.
#[derive(Debug, Clone)]
pub struct Fibonacci {
    curr: Duration,
    next: Duration,
}

impl Fibonacci {
    /// Creates a new `Fibonacci` using a random proportion of the given duration.
    pub fn new(duration: Duration) -> Fibonacci {
        let duration = jitter(duration);
        Fibonacci {
            curr: duration,
            next: duration,
        }
    }
    /// Creates a new `Fibonacci` using the given duration.
    pub fn exact(duration: Duration) -> Fibonacci {
        Fibonacci {
            curr: duration,
            next: duration,
        }
    }
}

impl Iterator for Fibonacci {
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        let duration = self.curr;

        let next = self.curr.saturating_add(self.next);
        self.curr = self.next;
        self.next = next;

        Some(duration)
    }
}

impl From<Duration> for Fibonacci {
    fn from(duration: Duration) -> Self {
        Self::new(duration)
    }
}

#[test]
fn fibonacci() {
    let mut iter = Fibonacci::exact(Duration::from_millis(10));
    assert_eq!(iter.next(), Some(Duration::from_millis(10)));
    assert_eq!(iter.next(), Some(Duration::from_millis(10)));
    assert_eq!(iter.next(), Some(Duration::from_millis(20)));
    assert_eq!(iter.next(), Some(Duration::from_millis(30)));
    assert_eq!(iter.next(), Some(Duration::from_millis(50)));
    assert_eq!(iter.next(), Some(Duration::from_millis(80)));
}

#[test]
fn fibonacci_saturated() {
    let mut iter = Fibonacci::exact(Duration::MAX);
    assert_eq!(iter.next(), Some(Duration::MAX));
    assert_eq!(iter.next(), Some(Duration::MAX));
}

/// Each retry uses a fixed delay.
#[derive(Debug, Clone)]
pub struct Fixed {
    duration: Duration,
}

impl Fixed {
    /// Creates a new `Fixed` using a random proportion of the given duration in milliseconds.
    pub fn new(duration: Duration) -> Self {
        Fixed {
            duration: jitter(duration),
        }
    }

    /// Creates a new `Fixed` using the given duration in milliseconds.
    pub fn exact(duration: Duration) -> Self {
        Fixed { duration }
    }
}

impl Iterator for Fixed {
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        Some(self.duration)
    }
}

impl From<Duration> for Fixed {
    fn from(duration: Duration) -> Self {
        Self { duration }
    }
}

/// Each retry happens immediately without any delay.
#[derive(Debug, Clone)]
pub struct NoDelay;

impl Iterator for NoDelay {
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        Some(Duration::default())
    }
}

#[cfg(test)]
mod test {
    use crate::delay::Exponential;
    use std::time::Duration;

    #[test]
    fn test_bounded_overflow() {
        let mut delays = Exponential::exact_with_factor(Duration::MAX, 1.0).bounded(Duration::MAX);

        assert_eq!(delays.next(), Some(Duration::MAX));

        assert_eq!(delays.next(), None);
    }
}
