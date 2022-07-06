use std::{
    ops::{Range as StdRange, RangeInclusive},
    time::Duration,
};

use rand::{
    distributions::{Distribution, Uniform},
    thread_rng,
};

/// Each retry uses a duration randomly chosen from a range. (need `random` feature)
#[derive(Debug, Clone)]
pub struct Range {
    distribution: Uniform<u64>,
}

impl Range {
    /// Create a new `Range` between the given millisecond durations, excluding the maximum value.
    ///
    /// # Panics
    ///
    /// Panics if the minimum is greater than or equal to the maximum.
    pub fn from_millis_exclusive(minimum: u64, maximum: u64) -> Self {
        Range {
            distribution: Uniform::new(minimum, maximum),
        }
    }

    /// Create a new `Range` between the given millisecond durations, including the maximum value.
    ///
    /// # Panics
    ///
    /// Panics if the minimum is greater than or equal to the maximum.
    pub fn from_millis_inclusive(minimum: u64, maximum: u64) -> Self {
        Range {
            distribution: Uniform::new_inclusive(minimum, maximum),
        }
    }
}

impl Iterator for Range {
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        Some(Duration::from_millis(
            self.distribution.sample(&mut thread_rng()),
        ))
    }
}

impl From<StdRange<Duration>> for Range {
    fn from(range: StdRange<Duration>) -> Self {
        Self::from_millis_exclusive(range.start.as_millis() as u64, range.end.as_millis() as u64)
    }
}

impl From<RangeInclusive<Duration>> for Range {
    fn from(range: RangeInclusive<Duration>) -> Self {
        Self::from_millis_inclusive(
            range.start().as_millis() as u64,
            range.end().as_millis() as u64,
        )
    }
}

/// Apply full random jitter to a duration. (need `random` feature)
pub fn jitter(duration: Duration) -> Duration {
    jitter_rng(duration, &mut thread_rng())
}

pub fn jitter_rng(duration: Duration, rng: &mut impl rand::Rng) -> Duration {
    duration.mul_f64(rng.gen())
}

#[cfg(test)]
mod test {
    use crate::delay::jitter_rng;
    use rand::SeedableRng;
    use rand_xorshift::XorShiftRng;
    use std::time::Duration;
    #[test]
    fn test_jitter_1_sec() {
        let mut rng = XorShiftRng::seed_from_u64(0);

        let duration = Duration::from_millis(1000);
        assert_ne!(
            jitter_rng(duration, &mut rng),
            jitter_rng(duration, &mut rng)
        )
    }
}
