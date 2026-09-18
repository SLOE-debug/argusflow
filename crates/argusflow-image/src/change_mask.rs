//! 懒分配的精确变化位图；逐字跳过空区域，不保存每个变化像素的额外索引。
pub(crate) struct ChangeMask {
    words: Vec<u64>,
    pixels: usize,
}
impl ChangeMask {
    pub(crate) fn new(pixels: usize) -> Self {
        Self {
            words: Vec::new(),
            pixels,
        }
    }
    pub(crate) fn insert(&mut self, index: usize) {
        if self.words.is_empty() {
            self.words.resize(self.pixels.div_ceil(64), 0);
        }
        self.words[index / 64] |= 1 << (index % 64);
    }
    pub(crate) fn take(&mut self, index: usize) -> bool {
        let word = &mut self.words[index / 64];
        let bit = 1 << (index % 64);
        let present = *word & bit != 0;
        *word &= !bit;
        present
    }
    /// 游标仅前进；连通区域遍历只清除位，不会在游标之前添加新变化。
    pub(crate) fn next(&self, cursor: &mut usize) -> Option<usize> {
        while let Some(&word) = self.words.get(*cursor) {
            if word != 0 {
                return Some(*cursor * 64 + word.trailing_zeros() as usize);
            }
            *cursor += 1;
        }
        None
    }
}
