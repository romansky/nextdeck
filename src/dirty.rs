use std::ops::{BitOr, BitOrAssign};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiDirty(u16);

impl UiDirty {
    pub const NONE: Self = Self(0);
    pub const TREE: Self = Self(1 << 0);
    pub const DETAILS: Self = Self(1 << 1);
    pub const OUTPUT: Self = Self(1 << 2);
    pub const STATUS: Self = Self(1 << 3);
    pub const MODAL: Self = Self(1 << 4);
    pub const LAYOUT: Self = Self(1 << 5);
    pub const THEME: Self = Self(1 << 6);
    pub const ALL: Self = Self(
        Self::TREE.0
            | Self::DETAILS.0
            | Self::OUTPUT.0
            | Self::STATUS.0
            | Self::MODAL.0
            | Self::LAYOUT.0
            | Self::THEME.0,
    );

    pub const fn any(self) -> bool {
        self.0 != 0
    }

    #[cfg(test)]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for UiDirty {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for UiDirty {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
