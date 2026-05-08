use crate::graphics_context::GraphicsContext;

pub trait SlotId: Sized {
    fn from_index(index: usize) -> Self;

    fn index(&self) -> usize;
}

pub struct GpuVec<T> {
    data: Vec<T>,
    gpu_buffer: Option<wgpu::Buffer>,
    pub(crate) gpu_buffer_len: usize,
    gpu_buffer_capacity: usize,
    buffer_resized: bool,
    dirty: bool,
    usage: wgpu::BufferUsages,
}

impl<T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable> GpuVec<T> {
    pub fn new(capacity: usize, usage: wgpu::BufferUsages) -> Self {
        Self {
            data: vec![T::default(); capacity],
            gpu_buffer: None,
            gpu_buffer_len: 0,
            gpu_buffer_capacity: 0,
            buffer_resized: false,
            dirty: false,
            usage,
        }
    }

    pub fn gpu_buffer(&self) -> &wgpu::Buffer {
        self.gpu_buffer
            .as_ref()
            .expect("Buffer has not been created")
    }

    pub fn data(&self) -> &[T] {
        &self.data[0..self.gpu_buffer_len]
    }

    pub fn take_buffer_resized(&mut self) -> bool {
        let was = self.buffer_resized;
        self.buffer_resized = false;

        was
    }

    pub fn len(&self) -> usize {
        self.gpu_buffer_len
    }

    pub fn is_empty(&self) -> bool {
        self.gpu_buffer_len == 0
    }

    pub fn push(&mut self, data: T) {
        let index = self.gpu_buffer_len;
        self.gpu_buffer_len += 1;

        if index >= self.data.len() {
            self.data.resize(self.data.len() * 2, T::default());
        }

        self.data[index] = data;
        self.dirty = true;
    }

    pub fn update(&mut self, index: usize, data: T) {
        // debug_assert!(self.ids.contains(&id), "Slot has not been allocated");
        if index >= self.gpu_buffer_len {
            panic!(
                "Out of bound, index: {}, len: {}",
                index, self.gpu_buffer_len
            );
        }

        self.data[index] = data;
        self.dirty = true;
    }

    pub fn clear(&mut self) {
        self.gpu_buffer_len = 0;
    }

    /// Ensures the GPU buffer exists and has enough capacity.
    ///
    /// On resize, flushes current data to the old buffer first so any draw
    /// commands already recorded against it see up-to-date values. wgpu's
    /// write_buffer staging guarantee ensures the write lands before those
    /// commands execute. The old buffer handle is then dropped — wgpu's
    /// internal refcount keeps the GPU resource alive until commands finish.
    pub fn ensure_capacity(&mut self, context: &GraphicsContext<'_, '_>) {
        if self.gpu_buffer.is_none() || self.gpu_buffer_len > self.gpu_buffer_capacity {
            // Write current data to the old buffer before replacing it.
            if let Some(old_buffer) = &self.gpu_buffer {
                context.queue.write_buffer(
                    old_buffer,
                    0,
                    bytemuck::cast_slice(&self.data[0..self.gpu_buffer_capacity]),
                );
            }

            self.gpu_buffer = Some(context.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (self.data.len() * std::mem::size_of::<T>()) as wgpu::BufferAddress,
                usage: self.usage,
                mapped_at_creation: false,
            }));
            self.gpu_buffer_capacity = self.data.len();
            self.buffer_resized = true;
            self.dirty = true;
        }
    }

    /// Ensures the buffer exists and uploads dirty data to the GPU.
    pub fn flush(&mut self, context: &GraphicsContext<'_, '_>) {
        self.ensure_capacity(context);

        if !self.dirty {
            return;
        }

        context.queue.write_buffer(
            self.gpu_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&self.data[0..self.gpu_buffer_len]),
        );

        self.dirty = false;
    }
}

pub struct GpuPoolBuffer<ID, T> {
    pub(crate) gpu: GpuVec<T>,
    pub(crate) free_ids: Vec<ID>,
    ids: Vec<ID>,
}

impl<ID, T> GpuPoolBuffer<ID, T>
where
    ID: SlotId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    pub fn new(capacity: usize, usage: wgpu::BufferUsages) -> Self {
        Self {
            gpu: GpuVec::new(capacity, usage),
            free_ids: Vec::with_capacity(capacity),
            ids: Vec::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn insert(&mut self, data: T) -> ID {
        let id = if self.free_ids.is_empty() {
            self.gpu.push(data);

            ID::from_index(self.gpu.len() - 1)
        } else {
            let index = self.free_ids.pop().unwrap().index();

            self.gpu.data[index] = data;
            self.gpu.dirty = true;

            ID::from_index(index)
        };

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
        self.gpu.clear();
        self.ids.clear();
        self.free_ids.clear();
    }

    pub fn ensure_capacity(&mut self, context: &GraphicsContext<'_, '_>) {
        self.gpu.ensure_capacity(context);
    }

    pub fn flush(&mut self, context: &GraphicsContext<'_, '_>) {
        self.gpu.flush(context);
    }

    pub fn ids(&self) -> &[ID] {
        &self.ids
    }

    pub fn gpu_buffer(&self) -> &wgpu::Buffer {
        self.gpu.gpu_buffer()
    }

    pub fn data(&self) -> &[T] {
        self.gpu.data()
    }

    pub fn take_buffer_resized(&mut self) -> bool {
        self.gpu.take_buffer_resized()
    }

    pub fn contains(&self, id: ID) -> bool {
        self.ids.contains(&id)
    }

    pub fn slot_data(&self, id: ID) -> &[T] {
        let idx = id.index();
        debug_assert!(idx < self.gpu.len(), "Slot not found");
        debug_assert!(!self.free_ids.contains(&id), "Slot has been removed");
        &self.gpu.data[idx..idx + 1]
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

    fn make_pool(capacity: usize) -> GpuPoolBuffer<TestId, i32> {
        GpuPoolBuffer::new(capacity, wgpu::BufferUsages::VERTEX)
    }

    #[test]
    fn test_pool_insert() {
        let mut buffer = make_pool(10);

        let id1 = buffer.insert(42);
        assert_eq!(id1.index(), 0);
        assert_eq!(buffer.slot_data(id1)[0], 42);
        assert_eq!(buffer.gpu.gpu_buffer_len, 1);

        let id2 = buffer.insert(24);
        assert_eq!(id2.index(), 1);
        assert_eq!(buffer.slot_data(id2)[0], 24);
        assert_eq!(buffer.gpu.gpu_buffer_len, 2);
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
        let mut buffer = make_pool(4);
        assert!(!buffer.take_buffer_resized());
    }

    #[test]
    fn test_pool_update() {
        let mut buffer = make_pool(10);

        let id = buffer.insert(42);
        buffer.update(id, 99);

        assert_eq!(buffer.slot_data(id)[0], 99);
    }

    #[test]
    fn test_pool_contains() {
        let mut buffer = make_pool(10);

        let id1 = buffer.insert(1);
        let id2 = buffer.insert(2);

        assert!(buffer.contains(id1));
        assert!(buffer.contains(id2));

        buffer.remove(id1);

        assert!(!buffer.contains(id1));
        assert!(buffer.contains(id2));
    }

    #[test]
    fn test_pool_len_tracks_active_slots() {
        let mut buffer = make_pool(10);

        assert_eq!(buffer.len(), 0);

        let id1 = buffer.insert(1);
        assert_eq!(buffer.len(), 1);

        buffer.insert(2);
        assert_eq!(buffer.len(), 2);

        buffer.remove(id1);
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_pool_is_empty() {
        let mut buffer = make_pool(10);

        assert!(buffer.is_empty());

        let id = buffer.insert(1);
        assert!(!buffer.is_empty());

        buffer.remove(id);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_pool_clear() {
        let mut buffer = make_pool(10);

        buffer.insert(1);
        buffer.insert(2);
        let id3 = buffer.insert(3);
        buffer.remove(id3);

        buffer.clear();

        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        assert!(buffer.free_ids.is_empty());
        assert_eq!(buffer.gpu.gpu_buffer_len, 0);
    }

    #[test]
    fn test_pool_gpu_len_does_not_shrink_on_remove() {
        let mut buffer = make_pool(10);

        buffer.insert(1);
        buffer.insert(2);
        let id3 = buffer.insert(3);

        assert_eq!(buffer.gpu.gpu_buffer_len, 3);

        buffer.remove(id3);

        // gpu_buffer_len covers the high-water mark; active len drops but gpu len stays
        assert_eq!(buffer.gpu.gpu_buffer_len, 3);
        assert_eq!(buffer.len(), 2);
    }

    // GpuVec tests --------------------------------------------------------------------------------

    fn make_gpu_vec(capacity: usize) -> GpuVec<i32> {
        GpuVec::new(capacity, wgpu::BufferUsages::VERTEX)
    }

    #[test]
    fn test_gpu_vec_push_increments_len() {
        let mut vec = make_gpu_vec(4);

        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());

        vec.push(10);
        assert_eq!(vec.len(), 1);
        assert!(!vec.is_empty());

        vec.push(20);
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn test_gpu_vec_data_reflects_pushes() {
        let mut vec = make_gpu_vec(4);

        vec.push(1);
        vec.push(2);
        vec.push(3);

        assert_eq!(vec.data(), &[1, 2, 3]);
    }

    #[test]
    fn test_gpu_vec_update() {
        let mut vec = make_gpu_vec(4);

        vec.push(1);
        vec.push(2);
        vec.update(0, 99);

        assert_eq!(vec.data()[0], 99);
        assert_eq!(vec.data()[1], 2);
    }

    #[test]
    #[should_panic]
    fn test_gpu_vec_update_out_of_bounds_panics() {
        let mut vec = make_gpu_vec(4);
        vec.push(1);
        vec.update(1, 99);
    }

    #[test]
    fn test_gpu_vec_clear() {
        let mut vec = make_gpu_vec(4);

        vec.push(1);
        vec.push(2);
        vec.clear();

        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
    }

    #[test]
    fn test_gpu_vec_grow() {
        let mut vec = make_gpu_vec(2);

        vec.push(1);
        vec.push(2);
        vec.push(3); // triggers resize

        assert_eq!(vec.len(), 3);
        assert_eq!(vec.data(), &[1, 2, 3]);
    }
}
