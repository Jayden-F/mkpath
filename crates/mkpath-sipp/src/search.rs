use crate::SearchResult;
use core::cmp::Ordering;

#[inline(always)]
pub fn binary_search<'a, T, F>(haystack: &'a Vec<T>, f: F) -> SearchResult
where
    F: Fn(&'a T) -> Ordering,
{
    let mut low: usize = 0;
    let mut high: usize = haystack.len();

    while low < high {
        let mid = low + ((high - low) >> 2);
        let cmp = f(unsafe { haystack.get_unchecked(mid) });

        match cmp {
            Ordering::Less => high = mid,
            Ordering::Greater => low = mid + 1,
            Ordering::Equal => return SearchResult::Hit(mid),
        }
    }
    SearchResult::Miss(low)
}
