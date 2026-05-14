use std::marker::PhantomData;
use std::ops::Range;

use crate::graphics_context::GraphicsContext;
use crate::memory::{GpuPoolBuffer, GpuVec, SlotId};

pub trait RenderInstances<ID, T> {
    type Drain: Iterator<Item = Range<u32>>;

    type Ranges: Iterator<Item = Range<u32>>;

    fn gpu_buffer(&self) -> &wgpu::Buffer;

    fn drain(&mut self, context: &GraphicsContext) -> Self::Drain;

    fn ranges(&mut self, context: &GraphicsContext) -> Self::Ranges;

    fn upload_all(&mut self, context: &GraphicsContext);

    fn data(&self) -> &[T];

    fn bind(&mut self, slot: u32, context: &GraphicsContext);
}

pub struct BumpRangesIter {
    range: Option<Range<u32>>,
}

impl Iterator for BumpRangesIter {
    type Item = Range<u32>;

    fn next(&mut self) -> Option<Range<u32>> {
        self.range.take()
    }
}

pub struct PoolDrainIter<ID: SlotId> {
    ids: Vec<ID>,
    cursor: usize,
}

impl<ID: SlotId> Iterator for PoolDrainIter<ID> {
    type Item = Range<u32>;

    fn next(&mut self) -> Option<Range<u32>> {
        if self.cursor >= self.ids.len() {
            return None;
        }

        let start = self.ids[self.cursor].index();
        let mut end = start + 1;
        self.cursor += 1;

        while self.cursor < self.ids.len() && self.ids[self.cursor].index() == end {
            end += 1;
            self.cursor += 1;
        }

        Some(start as u32..end as u32)
    }
}

pub struct PoolRangesIter<ID: SlotId> {
    free_ids: Vec<ID>,
    free_cursor: usize,
    cursor: usize,
    max_index: usize,
}

impl<ID: SlotId> Iterator for PoolRangesIter<ID> {
    type Item = Range<u32>;

    fn next(&mut self) -> Option<Range<u32>> {
        if self.cursor >= self.max_index {
            return None;
        }

        // Skip over consecutive free slots.
        while self.free_cursor < self.free_ids.len()
            && self.free_ids[self.free_cursor].index() == self.cursor
        {
            self.free_cursor += 1;
            self.cursor += 1;

            if self.cursor >= self.max_index {
                return None;
            }
        }

        // cursor is now at an active slot - find the end of this run.
        let start = self.cursor;

        loop {
            self.cursor += 1;

            if self.cursor >= self.max_index {
                return Some(start as u32..self.cursor as u32);
            }

            if self.free_cursor < self.free_ids.len()
                && self.free_ids[self.free_cursor].index() == self.cursor
            {
                return Some(start as u32..self.cursor as u32);
            }
        }
    }
}

pub struct BumpInstances<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    buffer: GpuVec<T>,
    drain_cursor: usize,
    phantom: PhantomData<ID>,
}

impl<ID, T> BumpInstances<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: GpuVec::new(
                capacity,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            ),
            drain_cursor: 0,
            phantom: PhantomData,
        }
    }

    pub fn insert(&mut self, data: T) -> ID {
        self.buffer.push(data);

        ID::from_index(self.buffer.len() - 1)
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.drain_cursor = 0;
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    fn ranges_iter(&self) -> BumpRangesIter {
        let len = self.buffer.len();
        BumpRangesIter {
            range: if len > 0 { Some(0..len as u32) } else { None },
        }
    }

    fn drain_iter(&mut self) -> BumpRangesIter {
        let start = self.drain_cursor;
        let end = self.buffer.len();
        self.drain_cursor = end;

        BumpRangesIter {
            range: if start < end {
                Some(start as u32..end as u32)
            } else {
                None
            },
        }
    }
}

impl<ID, T> RenderInstances<ID, T> for BumpInstances<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    type Drain = BumpRangesIter;
    type Ranges = BumpRangesIter;

    fn drain(&mut self, context: &GraphicsContext) -> BumpRangesIter {
        self.buffer.ensure_capacity(context);

        self.drain_iter()
    }

    fn ranges(&mut self, context: &GraphicsContext) -> BumpRangesIter {
        self.buffer.ensure_capacity(context);

        self.ranges_iter()
    }

    fn gpu_buffer(&self) -> &wgpu::Buffer {
        self.buffer.gpu_buffer()
    }

    fn upload_all(&mut self, context: &GraphicsContext) {
        self.buffer.flush(context);
    }

    fn data(&self) -> &[T] {
        self.buffer.data()
    }

    #[inline]
    fn bind(&mut self, slot: u32, context: &GraphicsContext) {
        self.buffer.ensure_capacity(context);

        context
            .render_pass()
            .set_vertex_buffer(slot, self.gpu_buffer().slice(..));
    }
}

pub struct PoolInstances<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    buffer: GpuPoolBuffer<ID, T>,
    pending: Vec<ID>,
}

impl<ID, T> PoolInstances<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: GpuPoolBuffer::new(
                capacity,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            ),
            pending: Vec::new(),
        }
    }

    pub fn insert(&mut self, data: T) -> ID {
        let id = self.buffer.insert(data);
        self.pending.push(id);

        id
    }

    pub fn remove(&mut self, id: ID) {
        self.buffer.remove(id);
        self.pending.retain(|&p| p != id);
    }

    pub fn update(&mut self, id: ID, data: T) {
        self.buffer.update(id, data);
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    fn ranges_iter(&self) -> PoolRangesIter<ID> {
        PoolRangesIter {
            free_ids: self.buffer.free_ids.clone(),
            free_cursor: 0,
            cursor: 0,
            max_index: self.buffer.gpu.gpu_buffer_len,
        }
    }

    fn drain_iter(&mut self) -> PoolDrainIter<ID> {
        let mut ids = std::mem::take(&mut self.pending);
        ids.sort_unstable_by_key(|id| id.index());

        PoolDrainIter { ids, cursor: 0 }
    }
}

impl<ID, T> RenderInstances<ID, T> for PoolInstances<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    type Drain = PoolDrainIter<ID>;

    type Ranges = PoolRangesIter<ID>;

    fn drain(&mut self, context: &GraphicsContext) -> PoolDrainIter<ID> {
        self.buffer.ensure_capacity(context);

        self.drain_iter()
    }

    fn ranges(&mut self, context: &GraphicsContext) -> PoolRangesIter<ID> {
        self.buffer.ensure_capacity(context);

        self.ranges_iter()
    }

    fn gpu_buffer(&self) -> &wgpu::Buffer {
        self.buffer.gpu_buffer()
    }

    fn upload_all(&mut self, context: &GraphicsContext) {
        self.buffer.flush(context);
    }

    fn data(&self) -> &[T] {
        self.buffer.data()
    }

    #[inline]
    fn bind(&mut self, slot: u32, context: &GraphicsContext) {
        self.buffer.ensure_capacity(context);

        context
            .render_pass()
            .set_vertex_buffer(slot, self.gpu_buffer().slice(..));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Copy, Clone, PartialEq, Debug)]
    struct TestId(usize);

    impl SlotId for TestId {
        fn from_index(index: usize) -> Self {
            TestId(index)
        }
        fn index(&self) -> usize {
            self.0
        }
    }

    // Helpers -------------------------------------------------------------------------------------

    fn drained(indices: &[usize]) -> Vec<Range<u32>> {
        let ids = indices.iter().map(|&i| TestId(i)).collect();

        PoolDrainIter { ids, cursor: 0 }.collect()
    }

    fn active(max_index: usize, free: &[usize]) -> Vec<Range<u32>> {
        let free_ids = free.iter().map(|&i| TestId(i)).collect();

        PoolRangesIter {
            free_ids,
            free_cursor: 0,
            cursor: 0,
            max_index,
        }
        .collect()
    }

    fn make_pool() -> PoolInstances<TestId, i32> {
        PoolInstances::new(8)
    }
    fn make_bump() -> BumpInstances<TestId, i32> {
        BumpInstances::new(8)
    }

    // BumpRangesIter ------------------------------------------------------------------------------

    #[test]
    fn bump_iter_empty() {
        assert_eq!(BumpRangesIter { range: None }.collect::<Vec<_>>(), vec![]);
    }

    #[test]
    fn bump_iter_single_range() {
        assert_eq!(
            BumpRangesIter { range: Some(0..4) }.collect::<Vec<_>>(),
            vec![0..4]
        );
    }

    #[test]
    fn bump_iter_yields_once() {
        let mut iter = BumpRangesIter { range: Some(2..7) };
        assert_eq!(iter.next(), Some(2..7));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    // PoolDrainIter -------------------------------------------------------------------------------

    #[test]
    fn drain_empty() {
        assert_eq!(drained(&[]), vec![]);
    }

    #[test]
    fn drain_single() {
        assert_eq!(drained(&[3]), vec![3..4]);
    }

    #[test]
    fn drain_contiguous() {
        assert_eq!(drained(&[0, 1, 2]), vec![0..3]);
    }

    #[test]
    fn drain_two_gaps() {
        assert_eq!(drained(&[0, 1, 3, 4]), vec![0..2, 3..5]);
    }

    #[test]
    fn drain_three_gaps() {
        assert_eq!(drained(&[0, 2, 5, 6]), vec![0..1, 2..3, 5..7]);
    }

    #[test]
    fn drain_all_isolated() {
        assert_eq!(drained(&[0, 2, 4]), vec![0..1, 2..3, 4..5]);
    }

    // PoolRangesIter ------------------------------------------------------------------------------

    #[test]
    fn active_none() {
        assert_eq!(active(0, &[]), vec![]);
    }

    #[test]
    fn active_no_free() {
        assert_eq!(active(4, &[]), vec![0..4]);
    }

    #[test]
    fn active_single_slot() {
        assert_eq!(active(1, &[]), vec![0..1]);
    }

    #[test]
    fn active_all_free() {
        assert_eq!(active(4, &[0, 1, 2, 3]), vec![]);
    }

    #[test]
    fn active_free_at_start() {
        assert_eq!(active(4, &[0]), vec![1..4]);
    }

    #[test]
    fn active_free_run_at_start() {
        assert_eq!(active(4, &[0, 1]), vec![2..4]);
    }

    #[test]
    fn active_free_at_end() {
        assert_eq!(active(4, &[3]), vec![0..3]);
    }

    #[test]
    fn active_free_run_at_end() {
        assert_eq!(active(6, &[4, 5]), vec![0..4]);
    }

    #[test]
    fn active_free_in_middle() {
        assert_eq!(active(6, &[2, 3]), vec![0..2, 4..6]);
    }

    #[test]
    fn active_multiple_gaps() {
        assert_eq!(active(5, &[1, 3]), vec![0..1, 2..3, 4..5]);
    }

    #[test]
    fn active_free_splits_run() {
        // slots: active 0..3, free 3, active 4, free 5
        assert_eq!(active(6, &[3, 5]), vec![0..3, 4..5]);
    }

    // PoolInstances state -------------------------------------------------------------------------

    #[test]
    fn pool_insert_adds_to_pending() {
        let mut instances = make_pool();
        let id = instances.insert(42);
        assert_eq!(instances.pending, vec![id]);
    }

    #[test]
    fn pool_multiple_inserts_all_pending() {
        let mut instances = make_pool();
        let id1 = instances.insert(1);
        let id2 = instances.insert(2);
        let id3 = instances.insert(3);
        assert_eq!(instances.pending, vec![id1, id2, id3]);
    }

    #[test]
    fn pool_remove_clears_from_pending() {
        let mut instances = make_pool();
        let id1 = instances.insert(1);
        let id2 = instances.insert(2);
        instances.remove(id1);
        assert_eq!(instances.pending, vec![id2]);
    }

    #[test]
    fn pool_slot_reuse_after_remove() {
        let mut instances = make_pool();
        let id1 = instances.insert(1);
        instances.insert(2);
        instances.remove(id1);
        let id3 = instances.insert(3);
        assert_eq!(id1.index(), id3.index());
        assert!(instances.pending.contains(&id3));
    }

    // BumpInstances state -------------------------------------------------------------------------

    #[test]
    fn bump_insert_increments_len() {
        let mut instances = make_bump();
        assert_eq!(instances.len(), 0);
        instances.insert(1);
        assert_eq!(instances.len(), 1);
        instances.insert(2);
        assert_eq!(instances.len(), 2);
    }

    #[test]
    fn bump_clear_resets_len_and_drain_cursor() {
        let mut instances = make_bump();
        instances.insert(1);
        instances.insert(2);
        instances.drain_cursor = 2;
        instances.clear();
        assert_eq!(instances.len(), 0);
        assert_eq!(instances.drain_cursor, 0);
    }

    #[test]
    fn bump_drain_cursor_advances() {
        let mut instances = make_bump();
        instances.insert(1);
        instances.insert(2);

        // First drain covers everything inserted so far.
        let start = instances.drain_cursor;
        instances.drain_cursor = instances.len();
        assert_eq!(start as u32..instances.drain_cursor as u32, 0..2);

        // After another insert, second drain covers only the new item.
        instances.insert(3);
        let start2 = instances.drain_cursor;
        instances.drain_cursor = instances.len();
        assert_eq!(start2 as u32..instances.drain_cursor as u32, 2..3);
    }

    #[test]
    fn bump_drain_cursor_empty_when_nothing_new() {
        let mut instances = make_bump();
        instances.insert(1);
        instances.drain_cursor = instances.len();
        assert_eq!(instances.drain_cursor, instances.len());
    }

    // Integration ---------------------------------------------------------------------------------

    #[test]
    fn pool_ranges_iter_after_remove() {
        let mut instances = make_pool();
        instances.insert(1);
        let id2 = instances.insert(2);
        let id3 = instances.insert(3);
        instances.insert(4);

        instances.remove(id2);
        instances.remove(id3);

        let ranges: Vec<Range<u32>> = instances.ranges_iter().collect();
        assert_eq!(ranges, vec![0..1, 3..4]);
    }

    #[test]
    fn pool_drain_iter_yields_only_pending() {
        let mut instances = make_pool();
        instances.insert(1);
        instances.insert(2);

        // Drain pending — clears pending list.
        let _: Vec<_> = instances.drain_iter().collect();

        // Insert one more; only the new item should appear in the next drain.
        instances.insert(3);
        let ranges: Vec<Range<u32>> = instances.drain_iter().collect();
        assert_eq!(ranges, vec![2..3]);
    }

    #[test]
    fn bump_ranges_iter_covers_all() {
        let mut instances = make_bump();
        instances.insert(1);
        instances.insert(2);
        instances.insert(3);

        let ranges: Vec<Range<u32>> = instances.ranges_iter().collect();
        assert_eq!(ranges, vec![0..3]);
    }

    #[test]
    fn bump_drain_iter_incremental() {
        let mut instances = make_bump();
        instances.insert(1);
        instances.insert(2);

        let first: Vec<Range<u32>> = instances.drain_iter().collect();
        assert_eq!(first, vec![0..2]);

        instances.insert(3);

        let second: Vec<Range<u32>> = instances.drain_iter().collect();
        assert_eq!(second, vec![2..3]);
    }
}
