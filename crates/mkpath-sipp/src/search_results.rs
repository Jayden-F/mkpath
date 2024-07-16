// Hit or miss
// I guess they never miss, huh?

#[derive(Debug, PartialEq)]
pub enum SearchResult {
    Hit(usize),
    Miss(usize),
}
