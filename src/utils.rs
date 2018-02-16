//! General utility functions.

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
