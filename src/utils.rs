//! General utility functions.

use std::time::SystemTime;

pub trait IteratorExt<T>: Iterator<Item = T> {
    /// Computes the minimum and maximum of the sequence simultaneously.
    fn minmax(self) -> Option<(T, T)>;
}

impl<
    T: Clone + PartialOrd + Sized,
    I: Iterator<Item = T> + Sized,
> IteratorExt<T> for I
{
    fn minmax(mut self) -> Option<(T, T)> {
        let min = |a, b| if a <= b { a } else { b };
        let max = |a, b| if a <= b { b } else { a };

        let (mut l, mut h) = match self.next() {
            None => return None,
            Some(e) => (e.clone(), e),
        };
        loop {
            let first = match self.next() {
                None => break,
                Some(e) => e,
            };
            let second = match self.next() {
                None => {
                    return Some(if first < l {
                        (first, h)
                    } else if h < first {
                        (l, first)
                    } else {
                        (l, h)
                    })
                }
                Some(e) => e,
            };
            if first < second {
                l = min(l, first);
                h = max(h, second);
            } else {
                l = min(l, second);
                h = max(h, first);
            }
        }
        Some((l, h))
    }
}

pub struct FpsCounter {
    value: f64,
    last_report: SystemTime,
    frames_since_report: u32,
}

/// Measures average FPS over 10-second windows.
///
/// After rendering a frame, call `rendered()` to update the value. If it
/// returns true, a new average is available.
impl FpsCounter {
    pub fn new() -> FpsCounter {
        FpsCounter {
            value: 0.0,
            last_report: SystemTime::now(),
            frames_since_report: 0,
        }
    }

    /// Update the value after rendering a frame.
    ///
    /// Returns true once every 10 seconds to signify that a new average is
    /// available.
    pub fn rendered(&mut self) -> bool {
        let now = SystemTime::now();
        let dt = match now.duration_since(self.last_report) {
            Ok(d) => d,
            Err(_) => {
                self.frames_since_report = 0;
                self.last_report = now;
                return false;
            }
        };
        if dt.as_secs() >= 10 {
            let duration =
                dt.as_secs() as f64 + dt.subsec_nanos() as f64 * 0.000_000_001;
            self.value = self.frames_since_report as f64 / duration;
            self.frames_since_report = 0;
            self.last_report = now;
            true
        } else {
            self.frames_since_report += 1;
            false
        }
    }

    /// Current average of frames per second (updated every 10 seconds).
    pub fn value(&self) -> f64 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::IteratorExt;

    #[test]
    fn test() {
        let r = [2, 4, 1, 5, 3].iter().minmax();
        assert_eq!(r, Some((&1, &5)));
        let r = [5, 4, 3, 2, 1].iter().minmax();
        assert_eq!(r, Some((&1, &5)));
        let r = [2, 2, 2].iter().minmax();
        assert_eq!(r, Some((&2, &2)));
        let r = [2].iter().minmax();
        assert_eq!(r, Some((&2, &2)));
        let r: Option<(&i32, &i32)> = [].iter().minmax();
        assert_eq!(r, None);
    }
}
