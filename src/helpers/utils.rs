//! A collection of helpful utility functions.

use rand::rngs::SmallRng;
use rand::seq::IteratorRandom;
use rand::SeedableRng;
use std::convert::TryFrom;
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current time in seconds since epoch.
pub fn epoch() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::default())
            .as_millis(),
    )
    .unwrap_or(0)
}

/// A trait to be implemented on iterators to allow conveniently sampling of set of random elements.
pub trait Sample {
    type Item;

    /// Returns a sample of `n` elements from the collection.
    fn sample(self, n: usize) -> Vec<Self::Item>;
}

/// Blanket implementation for all Iterators.
impl<T, I> Sample for T
where
    T: Iterator<Item = I>,
{
    type Item = I;

    /// Returns a random set of elements from this iterator.
    fn sample(self, n: usize) -> Vec<Self::Item> {
        let mut rng = SmallRng::from_entropy();
        self.choose_multiple(&mut rng, n)
    }
}
