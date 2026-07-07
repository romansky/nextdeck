#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RequestId(pub u64);

impl RequestId {
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}
