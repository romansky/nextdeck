#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StatusCounts {
    pub pending: usize,
    pub running: usize,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
    pub skipped: usize,
}
