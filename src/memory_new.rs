use std::{cmp::Ordering, ops::Range};

use wgpu::util::DeviceExt;

use crate::graphics_context::GraphicsContext;

pub trait SlotId: Sized {
    fn from_index(index: usize) -> Self;

    fn index(&self) -> usize;
}

pub trait GpuBuffer<T> {
    fn gpu_buffer(&self) -> &wgpu::Buffer;

    fn data(&self) -> &[T];

    fn upload_all(&mut self, context: &GraphicsContext<'_, '_>);

    fn take_buffer_resized(&mut self) -> bool;
}

pub trait SlottedBuffer<ID, T>: GpuBuffer<T> {
    fn upload_slot(&mut self, context: &GraphicsContext<'_, '_>, id: ID);

    fn contains(&self, id: ID) -> bool;

    fn ids(&self) -> &[ID];

    fn slot_data(&self, id: ID) -> &[T];
}

pub struct RangesIter<ID> {
    removed_ids: Vec<ID>,
    current_id_index: usize,
    len: usize,
    last_index: usize,
}

impl<ID: SlotId> Iterator for RangesIter<ID> {
    type Item = Range<u32>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current_id_index.cmp(&self.removed_ids.len()) {
            Ordering::Less => {
                let next_index = self.removed_ids[self.current_id_index].index();

                if self.last_index == next_index {
                    self.current_id_index += 1;
                    self.last_index = self.current_id_index;

                    return self.next();
                }

                // Skip consequent removed indexes
                let mut idx = self.removed_ids[self.current_id_index].index();
                self.current_id_index += 1;

                while self.current_id_index < self.removed_ids.len()
                    && self.removed_ids[self.current_id_index].index() - idx == 1
                {
                    idx = self.removed_ids[self.current_id_index].index();
                    self.current_id_index += 1;
                }

                let range = self.last_index as u32..next_index as u32;
                self.last_index = idx + 1;

                Some(range)
            }
            Ordering::Equal => {
                if self.len == 0 || self.removed_ids.last().map(|v| v.index()) == Some(self.len - 1)
                {
                    None
                } else {
                    self.current_id_index += 1;

                    Some(self.last_index as u32..self.len as u32)
                }
            }
            Ordering::Greater => None,
        }
    }
}

pub struct GpuVec<T> {
    data: Vec<T>,
    gpu_buffer: Option<wgpu::Buffer>,
    gpu_capacity: usize,
    buffer_resized: bool,
    dirty: bool,
    usage: wgpu::BufferUsages,
}

impl<T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable> GpuVec<T> {
    pub fn new(capacity: usize, usage: wgpu::BufferUsages) -> Self {
        Self {
            data: vec![T::default(); capacity],
            gpu_buffer: None,
            gpu_capacity: 0,
            buffer_resized: false,
            dirty: false,
            usage,
        }
    }

    pub fn ensure_created(&mut self, context: &GraphicsContext<'_, '_>) {
        if self.gpu_buffer.is_some() {
            return;
        }

        let gpu_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&self.data),
                usage: self.usage,
            });

        self.gpu_buffer = Some(gpu_buffer);
        self.gpu_capacity = self.data.len();
    }

    pub fn recreate(&mut self, context: &GraphicsContext<'_, '_>) {
        let gpu_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&self.data),
                usage: self.usage,
            });

        self.gpu_buffer = Some(gpu_buffer);
        self.gpu_capacity = self.data.len();
        self.buffer_resized = true;
    }

    pub fn upload_all(&mut self, context: &GraphicsContext<'_, '_>, active_len: usize) {
        self.ensure_created(context);

        if active_len > self.gpu_capacity {
            self.recreate(context);
        }

        context.queue.write_buffer(
            self.gpu_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&self.data[0..active_len]),
        );
    }

    pub fn upload_slot(&mut self, context: &GraphicsContext<'_, '_>, slot: usize, active_len: usize) {
        self.ensure_created(context);

        if active_len > self.gpu_capacity {
            self.recreate(context);

            // Full upload since the buffer was recreated.
            context.queue.write_buffer(
                self.gpu_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&self.data[0..active_len]),
            );
            return;
        }

        let byte_offset = (slot * std::mem::size_of::<T>()) as wgpu::BufferAddress;
        context.queue.write_buffer(
            self.gpu_buffer.as_ref().unwrap(),
            byte_offset,
            bytemuck::cast_slice(&self.data[slot..slot + 1]),
        );
    }

    pub fn gpu_buffer(&self) -> &wgpu::Buffer {
        self.gpu_buffer
            .as_ref()
            .expect("Buffer has not been created")
    }

    pub fn data(&self, active_len: usize) -> &[T] {
        &self.data[0..active_len]
    }

    pub fn take_buffer_resized(&mut self) -> bool {
        let was = self.buffer_resized;
        self.buffer_resized = false;

        was
    }

    /// Ensures the GPU buffer exists and has enough capacity.
    /// Does NOT upload data — call before recording draw commands.
    pub fn ensure_capacity(&mut self, context: &GraphicsContext<'_, '_>, active_len: usize) {
        if self.gpu_buffer.is_none() {
            self.gpu_buffer = Some(context.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (self.data.len() * std::mem::size_of::<T>()) as wgpu::BufferAddress,
                usage: self.usage,
                mapped_at_creation: false,
            }));
            self.gpu_capacity = self.data.len();
            self.buffer_resized = true;
        } else if active_len > self.gpu_capacity {
            self.gpu_buffer = Some(context.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (self.data.len() * std::mem::size_of::<T>()) as wgpu::BufferAddress,
                usage: self.usage,
                mapped_at_creation: false,
            }));
            self.gpu_capacity = self.data.len();
            self.buffer_resized = true;
        }
    }

    /// Uploads dirty data to the GPU. Does NOT resize — call ensure_capacity first.
    pub fn flush(&mut self, context: &GraphicsContext<'_, '_>, active_len: usize) {
        if !self.dirty {
            return;
        }
        let Some(buffer) = &self.gpu_buffer else { return };
        context.queue.write_buffer(
            buffer,
            0,
            bytemuck::cast_slice(&self.data[0..active_len]),
        );
        self.dirty = false;
    }
}

pub struct BumpBuffer<ID, T> {
    gpu: GpuVec<T>,
    ids: Vec<ID>,
    max_index: usize,
}

impl<ID, T> BumpBuffer<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    pub fn new(capacity: usize, usage: wgpu::BufferUsages) -> Self {
        Self {
            gpu: GpuVec::new(capacity, usage),
            ids: Vec::with_capacity(capacity),
            max_index: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn insert(&mut self, data: T) -> ID {
        let index = self.max_index;
        self.max_index += 1;

        if index >= self.gpu.data.len() {
            self.gpu.data.resize(self.gpu.data.len() * 2, T::default());
        }

        let id = ID::from_index(index);
        self.gpu.data[index] = data;
        self.gpu.dirty = true;
        self.ids.push(id);

        id
    }

    pub fn update(&mut self, id: ID, data: T) {
        debug_assert!(self.ids.contains(&id), "Slot has not been allocated");
        self.gpu.data[id.index()] = data;
        self.gpu.dirty = true;
    }

    pub fn clear(&mut self) {
        self.max_index = 0;
        self.ids.clear();
    }

    pub fn ensure_capacity(&mut self, context: &GraphicsContext<'_, '_>) {
        self.gpu.ensure_capacity(context, self.max_index);
    }

    pub fn flush(&mut self, context: &GraphicsContext<'_, '_>) {
        self.gpu.flush(context, self.max_index);
    }

    pub fn ranges_iter(&self) -> RangesIter<ID> {
        RangesIter {
            removed_ids: vec![],
            current_id_index: 0,
            len: self.max_index,
            last_index: 0,
        }
    }
}

impl<ID, T> GpuBuffer<T> for BumpBuffer<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    fn gpu_buffer(&self) -> &wgpu::Buffer {
        self.gpu.gpu_buffer()
    }

    fn data(&self) -> &[T] {
        self.gpu.data(self.max_index)
    }

    fn upload_all(&mut self, context: &GraphicsContext<'_, '_>) {
        self.gpu.upload_all(context, self.max_index);
    }

    fn take_buffer_resized(&mut self) -> bool {
        self.gpu.take_buffer_resized()
    }
}

impl<ID, T> SlottedBuffer<ID, T> for BumpBuffer<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    fn upload_slot(&mut self, context: &GraphicsContext<'_, '_>, id: ID) {
        self.gpu.upload_slot(context, id.index(), self.max_index);
    }

    fn contains(&self, id: ID) -> bool {
        self.ids.contains(&id)
    }

    fn ids(&self) -> &[ID] {
        &self.ids
    }

    fn slot_data(&self, id: ID) -> &[T] {
        let idx = id.index();
        debug_assert!(idx < self.max_index, "Slot not found");

        &self.gpu.data[idx..idx + 1]
    }
}

pub struct PoolBuffer<ID, T> {
    pub(crate) gpu: GpuVec<T>,
    pub(crate) free_ids: Vec<ID>,
    ids: Vec<ID>,
    pub(crate) max_index: usize,
}

impl<ID, T> PoolBuffer<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    pub fn new(capacity: usize, usage: wgpu::BufferUsages) -> Self {
        Self {
            gpu: GpuVec::new(capacity, usage),
            free_ids: Vec::with_capacity(capacity),
            ids: Vec::with_capacity(capacity),
            max_index: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn insert(&mut self, data: T) -> ID {
        let index = if self.free_ids.is_empty() {
            let idx = self.max_index;
            self.max_index += 1;

            if idx >= self.gpu.data.len() {
                self.gpu.data.resize(self.gpu.data.len() * 2, T::default());
            }

            idx
        } else {
            self.free_ids.pop().unwrap().index()
        };

        let id = ID::from_index(index);
        self.gpu.data[index] = data;
        self.gpu.dirty = true;
        self.ids.push(id);

        id
    }

    pub fn remove(&mut self, id: ID) {
        let index = self
            .ids
            .iter()
            .position(|other| *other == id)
            .expect("Id not found");
        self.ids.remove(index);
        self.free_ids.push(id);
        self.free_ids.sort_by_key(|a| a.index());
    }

    pub fn update(&mut self, id: ID, data: T) {
        debug_assert!(self.ids.contains(&id), "Slot has not been allocated");
        self.gpu.data[id.index()] = data;
        self.gpu.dirty = true;
    }

    pub fn clear(&mut self) {
        self.max_index = 0;
        self.ids.clear();
        self.free_ids.clear();
    }

    pub fn ensure_capacity(&mut self, context: &GraphicsContext<'_, '_>) {
        self.gpu.ensure_capacity(context, self.max_index);
    }

    pub fn flush(&mut self, context: &GraphicsContext<'_, '_>) {
        self.gpu.flush(context, self.max_index);
    }

    pub fn ranges_iter(&self) -> RangesIter<ID> {
        RangesIter {
            removed_ids: self.free_ids.clone(),
            current_id_index: 0,
            len: self.max_index,
            last_index: 0,
        }
    }
}

impl<ID, T> GpuBuffer<T> for PoolBuffer<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    fn gpu_buffer(&self) -> &wgpu::Buffer {
        self.gpu.gpu_buffer()
    }

    fn data(&self) -> &[T] {
        self.gpu.data(self.max_index)
    }

    fn upload_all(&mut self, context: &GraphicsContext<'_, '_>) {
        self.gpu.upload_all(context, self.max_index);
    }

    fn take_buffer_resized(&mut self) -> bool {
        self.gpu.take_buffer_resized()
    }
}

impl<ID, T> SlottedBuffer<ID, T> for PoolBuffer<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    fn upload_slot(&mut self, context: &GraphicsContext<'_, '_>, id: ID) {
        self.gpu.upload_slot(context, id.index(), self.max_index);
    }

    fn contains(&self, id: ID) -> bool {
        self.ids.contains(&id)
    }

    fn ids(&self) -> &[ID] {
        &self.ids
    }

    fn slot_data(&self, id: ID) -> &[T] {
        let idx = id.index();
        debug_assert!(idx < self.max_index, "Slot not found");
        debug_assert!(!self.free_ids.contains(&id), "Slot has been removed");
        &self.gpu.data[idx..idx + 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Range;

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

    fn make_pool(capacity: usize) -> PoolBuffer<TestId, i32> {
        PoolBuffer::new(capacity, wgpu::BufferUsages::VERTEX)
    }

    fn make_bump(capacity: usize) -> BumpBuffer<TestId, i32> {
        BumpBuffer::new(capacity, wgpu::BufferUsages::VERTEX)
    }

    #[test]
    fn test_pool_insert() {
        let mut buffer = make_pool(10);

        let id1 = buffer.insert(42);
        assert_eq!(id1.index(), 0);
        assert_eq!(buffer.slot_data(id1)[0], 42);
        assert_eq!(buffer.max_index, 1);

        let id2 = buffer.insert(24);
        assert_eq!(id2.index(), 1);
        assert_eq!(buffer.slot_data(id2)[0], 24);
        assert_eq!(buffer.max_index, 2);
    }

    #[test]
    fn test_pool_remove() {
        let mut buffer = make_pool(10);

        let id1 = buffer.insert(42);
        let id2 = buffer.insert(24);

        buffer.remove(id1);
        assert!(!buffer.ids().contains(&id1));
        assert!(buffer.ids().contains(&id2));
        assert_eq!(buffer.free_ids.len(), 1);
        assert_eq!(buffer.free_ids[0], id1);
    }

    #[test]
    fn test_pool_slot_reuse() {
        let mut buffer = make_pool(10);

        let id1 = buffer.insert(42);
        buffer.remove(id1);

        let id2 = buffer.insert(24);
        assert_eq!(id1.index(), id2.index());
        assert!(buffer.free_ids.is_empty());
    }

    #[test]
    fn test_pool_ranges_iter() {
        let mut buffer = make_pool(10);

        buffer.insert(1);
        let id2 = buffer.insert(2);
        let id3 = buffer.insert(3);
        buffer.insert(4);

        buffer.remove(id2);
        buffer.remove(id3);

        let ranges: Vec<Range<u32>> = buffer.ranges_iter().collect();

        println!("{:?}", ranges);

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], (0..1));
        assert_eq!(ranges[1], (3..4));
    }

    #[test]
    fn test_pool_ranges_empty() {
        let buffer = make_pool(10);
        let ranges: Vec<Range<u32>> = buffer.ranges_iter().collect();

        assert_eq!(ranges.len(), 0);
    }

    #[test]
    fn test_pool_ranges_remove_last_item() {
        let mut buffer = make_pool(10);

        buffer.insert(1);
        let id2 = buffer.insert(2);
        buffer.insert(3);
        let id4 = buffer.insert(4);

        buffer.remove(id2);
        buffer.remove(id4);

        let ranges: Vec<Range<u32>> = buffer.ranges_iter().collect();

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], (0..1));
        assert_eq!(ranges[1], (2..3));
    }

    #[test]
    fn test_pool_ranges_all_removed() {
        let mut buf = make_pool(10);

        let id1 = buf.insert(1);
        let id2 = buf.insert(2);
        let id3 = buf.insert(3);
        let id4 = buf.insert(4);

        buf.remove(id1);
        buf.remove(id2);
        buf.remove(id3);
        buf.remove(id4);

        let ranges: Vec<Range<u32>> = buf.ranges_iter().collect();
        assert_eq!(ranges.len(), 0);
    }

    #[test]
    fn test_pool_ranges_no_removes() {
        let mut buffer = make_pool(10);

        buffer.insert(1);
        buffer.insert(2);
        buffer.insert(3);
        buffer.insert(4);

        let ranges: Vec<Range<u32>> = buffer.ranges_iter().collect();

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (0..4));
    }

    #[test]
    fn test_pool_ranges_remove_first() {
        let mut buffer = make_pool(10);

        let id1 = buffer.insert(1);
        buffer.insert(2);
        buffer.insert(3);
        buffer.insert(4);

        buffer.remove(id1);

        let ranges: Vec<Range<u32>> = buffer.ranges_iter().collect();

        println!("RANGES: {:?}", ranges);

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (1..4));
    }

    #[test]
    fn test_pool_ranges_remove_last() {
        let mut buffer = make_pool(10);

        buffer.insert(1);
        buffer.insert(2);
        buffer.insert(3);
        let id4 = buffer.insert(4);

        buffer.remove(id4);

        let ranges: Vec<Range<u32>> = buffer.ranges_iter().collect();

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (0..3));
    }

    #[test]
    fn test_pool_ranges_remove_consequent_first() {
        let mut buffer = make_pool(10);

        let id1 = buffer.insert(1);
        let id2 = buffer.insert(2);
        buffer.insert(3);
        buffer.insert(4);

        buffer.remove(id1);
        buffer.remove(id2);

        let ranges: Vec<Range<u32>> = buffer.ranges_iter().collect();

        println!("RANGES: {:?}", ranges);

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (2..4));
    }

    #[test]
    fn test_pool_ranges_remove_consequent_last() {
        let mut buffer = make_pool(10);

        buffer.insert(1);
        buffer.insert(2);
        let id3 = buffer.insert(3);
        let id4 = buffer.insert(4);
        buffer.insert(5);
        let id6 = buffer.insert(6);

        buffer.remove(id3);
        buffer.remove(id4);
        buffer.remove(id6);

        let ranges: Vec<Range<u32>> = buffer.ranges_iter().collect();

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], (0..2));
        assert_eq!(ranges[1], (4..5));
    }

    #[test]
    fn test_bump_insert() {
        let mut buffer = make_bump(10);

        let id1 = buffer.insert(42);
        assert_eq!(id1.index(), 0);
        assert_eq!(buffer.slot_data(id1)[0], 42);
        assert_eq!(buffer.max_index, 1);

        let id2 = buffer.insert(24);
        assert_eq!(id2.index(), 1);
        assert_eq!(buffer.slot_data(id2)[0], 24);
        assert_eq!(buffer.max_index, 2);
    }

    #[test]
    fn test_bump_ranges_iter() {
        let mut buffer = make_bump(10);

        buffer.insert(1);
        buffer.insert(2);
        buffer.insert(3);
        buffer.insert(4);

        let ranges: Vec<Range<u32>> = buffer.ranges_iter().collect();

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (0..4));
    }

    #[test]
    fn test_bump_clear() {
        let mut buffer = make_bump(10);

        buffer.insert(1);
        buffer.insert(2);

        buffer.clear();

        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_bump_grow() {
        let mut buffer = make_bump(2);

        buffer.insert(1);
        buffer.insert(2);
        buffer.insert(3);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.slot_data(TestId(2))[0], 3);
    }

    #[test]
    fn test_pool_grow() {
        let mut buffer = make_pool(2);

        buffer.insert(1);
        buffer.insert(2);
        buffer.insert(3);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.slot_data(TestId(2))[0], 3);
    }

    #[test]
    fn test_pool_slot_reuse_no_grow() {
        let mut buffer = make_pool(2);

        let id0 = buffer.insert(1);
        buffer.insert(2);
        buffer.remove(id0);
        let id_reused = buffer.insert(99);

        assert_eq!(id_reused.index(), 0);
        assert_eq!(buffer.slot_data(id_reused)[0], 99);
    }

    #[test]
    fn test_buffer_resized_false_initially() {
        let mut buffer = make_bump(4);
        assert!(!buffer.take_buffer_resized());
    }
}
