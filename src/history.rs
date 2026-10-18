//! Fixed-capacity ring buffer for metric history.
//!
//! The buffer never grows after construction, so pushing a sample is
//! allocation-free. Iteration yields oldest → newest.

#[derive(Debug, Clone)]
pub struct Ring<T> {
    buf: Box<[T]>,
    /// Index where the next sample is written.
    head: usize,
    len: usize,
}

impl<T: Copy + Default> Ring<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "ring capacity must be > 0");
        Self {
            buf: vec![T::default(); capacity].into_boxed_slice(),
            head: 0,
            len: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, value: T) {
        let cap = self.buf.len();
        self.buf[self.head] = value;
        self.head = (self.head + 1) % cap;
        if self.len < cap {
            self.len += 1;
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Most recent sample, if any.
    pub fn last(&self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            let cap = self.buf.len();
            Some(self.buf[(self.head + cap - 1) % cap])
        }
    }

    /// The last `n` samples (fewer if not enough yet), oldest → newest.
    pub fn iter_last(
        &self,
        n: usize,
    ) -> impl DoubleEndedIterator<Item = T> + ExactSizeIterator + '_ {
        let cap = self.buf.len();
        let m = n.min(self.len);
        let start = (self.head + cap - m) % cap;
        (0..m).map(move |i| self.buf[(start + i) % cap])
    }
}

impl<T: Copy + Default + Ord> Ring<T> {
    /// Maximum over the last `n` samples.
    pub fn max_last(&self, n: usize) -> Option<T> {
        self.iter_last(n).max()
    }
}

impl Ring<u64> {
    /// Mean over the last `n` samples (integer division).
    pub fn avg_last(&self, n: usize) -> Option<u64> {
        let m = n.min(self.len);
        if m == 0 {
            return None;
        }
        let sum: u128 = self.iter_last(m).map(u128::from).sum();
        Some((sum / m as u128) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_iterate_in_order() {
        let mut r = Ring::new(4);
        assert!(r.is_empty());
        for v in 1..=3u64 {
            r.push(v);
        }
        assert_eq!(r.len(), 3);
        assert_eq!(r.iter_last(10).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(r.last(), Some(3));
    }

    #[test]
    fn wraps_and_keeps_newest() {
        let mut r = Ring::new(3);
        for v in 1..=7u64 {
            r.push(v);
        }
        assert_eq!(r.len(), 3);
        assert_eq!(r.iter_last(3).collect::<Vec<_>>(), vec![5, 6, 7]);
        assert_eq!(r.iter_last(2).collect::<Vec<_>>(), vec![6, 7]);
        assert_eq!(r.iter_last(0).count(), 0);
    }

    #[test]
    fn max_and_avg_over_window() {
        let mut r = Ring::new(8);
        for v in [5u64, 9, 1, 3] {
            r.push(v);
        }
        assert_eq!(r.max_last(8), Some(9));
        assert_eq!(r.max_last(2), Some(3));
        assert_eq!(r.avg_last(4), Some(4));
        assert_eq!(r.avg_last(1), Some(3));
        let empty: Ring<u64> = Ring::new(2);
        assert_eq!(empty.max_last(2), None);
        assert_eq!(empty.avg_last(2), None);
    }

    #[test]
    fn clear_resets() {
        let mut r = Ring::new(2);
        r.push(1u64);
        r.clear();
        assert!(r.is_empty());
        assert_eq!(r.last(), None);
        r.push(4);
        assert_eq!(r.iter_last(2).collect::<Vec<_>>(), vec![4]);
    }
}
