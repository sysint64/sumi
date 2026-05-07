use std::{cmp::Ordering, ops::Range};

use wgpu::util::DeviceExt;

use crate::graphics_context::{GraphicsContext, LoadToGPUSchedule};

pub trait InstanceId {
    fn index(&self) -> usize;
}

pub trait Instances<ID, T> {
    fn create_buffer(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        usage: wgpu::BufferUsages,
        id_factory: fn(usize, &T) -> ID,
    );

    fn ranges_iter(&self) -> InstancesRangesIter<ID>;

    fn contains(&self, id: ID) -> bool;

    fn ids(&self) -> &[ID];

    fn gpu_buffer(&self) -> &wgpu::Buffer;

    fn occupied_buffer(&self) -> &[T];

    fn full_buffer_data(&self) -> &[T];

    fn instance_buffer(&self, id: ID) -> &[T];

    fn load_instance_to_gpu(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        schedule: LoadToGPUSchedule,
        id: ID,
    );

    fn load_all_instances_to_gpu(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        schedule: LoadToGPUSchedule,
    );

    fn take_buffer_resized(&mut self) -> bool;
}

pub struct PoolInstances<ID, T> {
    buffer_data: Vec<T>,
    gpu_buffer: Option<wgpu::Buffer>,
    removed_ids: Vec<ID>,
    ids: Vec<ID>,
    max_index: usize,
    id_factory: Option<fn(usize, &T) -> ID>,
    gpu_capacity: usize,
    buffer_resized: bool,
}

pub struct BumpInstances<ID, T> {
    buffer_data: Vec<T>,
    gpu_buffer: Option<wgpu::Buffer>,
    ids: Vec<ID>,
    max_index: usize,
    id_factory: Option<fn(usize, &T) -> ID>,
    gpu_capacity: usize,
    buffer_resized: bool,
}

pub struct InstancesRangesIter<ID> {
    removed_ids: Vec<ID>,
    current_id_index: usize,
    len: usize,
    last_index: usize,
}

impl<ID: InstanceId> Iterator for InstancesRangesIter<ID> {
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
                self.last_index = self.current_id_index + 1;

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

impl<ID, T> Instances<ID, T> for BumpInstances<ID, T>
where
    ID: InstanceId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    fn create_buffer(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        usage: wgpu::BufferUsages,
        id_factory: fn(usize, &T) -> ID,
    ) {
        debug_assert!(self.gpu_buffer.is_none(), "Buffer already has created");

        let gpu_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SVG Shader Instance Buffer"),
                contents: bytemuck::cast_slice(self.full_buffer_data()),
                usage,
            });

        self.id_factory = Some(id_factory);
        self.gpu_buffer = Some(gpu_buffer);
        self.gpu_capacity = self.buffer_data.len();
    }

    fn load_instance_to_gpu(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        schedule: LoadToGPUSchedule,
        id: ID,
    ) {
        if self.max_index > self.gpu_capacity {
            self.recreate_gpu_buffer(context);
            context.queue.write_buffer(
                self.gpu_buffer(),
                0,
                bytemuck::cast_slice(self.occupied_buffer()),
            );
            if schedule == LoadToGPUSchedule::Immediately {
                context.queue.submit([]);
            }
            return;
        }

        let instance_size = std::mem::size_of::<T>();
        let byte_offset =
            (id.index() as wgpu::BufferAddress) * (instance_size as wgpu::BufferAddress);

        context.queue.write_buffer(
            self.gpu_buffer(),
            byte_offset,
            bytemuck::cast_slice(self.instance_buffer(id)),
        );

        if schedule == LoadToGPUSchedule::Immediately {
            context.queue.submit([]);
        }
    }

    fn load_all_instances_to_gpu(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        schedule: LoadToGPUSchedule,
    ) {
        if self.max_index > self.gpu_capacity {
            self.recreate_gpu_buffer(context);
        }

        context.queue.write_buffer(
            self.gpu_buffer(),
            0,
            bytemuck::cast_slice(self.occupied_buffer()),
        );

        if schedule == LoadToGPUSchedule::Immediately {
            context.queue.submit([]);
        }
    }

    fn take_buffer_resized(&mut self) -> bool {
        let was = self.buffer_resized;
        self.buffer_resized = false;
        was
    }

    #[inline]
    fn gpu_buffer(&self) -> &wgpu::Buffer {
        self.gpu_buffer
            .as_ref()
            .expect("Buffer has not been created")
    }

    fn ranges_iter(&self) -> InstancesRangesIter<ID> {
        InstancesRangesIter {
            removed_ids: vec![],
            current_id_index: 0,
            len: self.max_index,
            last_index: 0,
        }
    }

    fn contains(&self, id: ID) -> bool {
        self.ids.contains(&id)
    }

    fn ids(&self) -> &[ID] {
        &self.ids
    }

    fn occupied_buffer(&self) -> &[T] {
        &self.buffer_data[0..self.max_index]
    }

    fn full_buffer_data(&self) -> &[T] {
        &self.buffer_data
    }

    fn instance_buffer(&self, id: ID) -> &[T] {
        let idx = id.index();

        debug_assert!(idx < self.max_index, "Instance not found");

        &self.buffer_data[idx..(idx + 1)]
    }
}

impl<ID, T> BumpInstances<ID, T>
where
    ID: InstanceId + Copy + Clone + PartialEq,
    T: Default + Clone,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer_data: vec![T::default(); capacity],
            ids: Vec::with_capacity(capacity),
            max_index: 0,
            id_factory: None,
            gpu_buffer: None,
            gpu_capacity: 0,
            buffer_resized: false,
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn insert(&mut self, data: T) -> ID {
        let id_factory = self
            .id_factory
            .expect("Instances has not been initizalized");

        let index = self.max_index;
        self.max_index += 1;

        if index >= self.buffer_data.len() {
            self.buffer_data
                .resize(self.buffer_data.len() * 2, T::default());
        }

        let id = id_factory(index, &data);
        self.buffer_data[index] = data;

        self.ids.push(id);

        id
    }

    fn recreate_gpu_buffer(&mut self, context: &GraphicsContext<'_, '_>)
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        let gpu_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SVG Shader Instance Buffer"),
                contents: bytemuck::cast_slice(&self.buffer_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        self.gpu_buffer = Some(gpu_buffer);
        self.gpu_capacity = self.buffer_data.len();
        self.buffer_resized = true;
    }

    pub fn update_instance(&mut self, id: ID, data: T) {
        debug_assert!(self.ids.contains(&id), "Instance has not been allocated");

        self.buffer_data[id.index()] = data;
    }

    pub fn clear(&mut self) {
        self.max_index = 0;
        self.ids.clear();
    }
}

impl<ID, T> Instances<ID, T> for PoolInstances<ID, T>
where
    ID: InstanceId + Copy + Clone + PartialEq,
    T: Default + Clone + bytemuck::Pod + bytemuck::Zeroable,
{
    fn create_buffer(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        usage: wgpu::BufferUsages,
        id_factory: fn(usize, &T) -> ID,
    ) {
        debug_assert!(self.gpu_buffer.is_none(), "Buffer already has created");

        let gpu_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SVG Shader Instance Buffer"),
                contents: bytemuck::cast_slice(self.full_buffer_data()),
                usage,
            });

        self.id_factory = Some(id_factory);
        self.gpu_buffer = Some(gpu_buffer);
        self.gpu_capacity = self.buffer_data.len();
    }

    fn load_instance_to_gpu(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        schedule: LoadToGPUSchedule,
        id: ID,
    ) {
        if self.max_index > self.gpu_capacity {
            self.recreate_gpu_buffer(context);
            context.queue.write_buffer(
                self.gpu_buffer(),
                0,
                bytemuck::cast_slice(self.occupied_buffer()),
            );
            if schedule == LoadToGPUSchedule::Immediately {
                context.queue.submit([]);
            }
            return;
        }

        let instance_size = std::mem::size_of::<T>();
        let byte_offset =
            (id.index() as wgpu::BufferAddress) * (instance_size as wgpu::BufferAddress);

        context.queue.write_buffer(
            self.gpu_buffer(),
            byte_offset,
            bytemuck::cast_slice(self.instance_buffer(id)),
        );

        if schedule == LoadToGPUSchedule::Immediately {
            context.queue.submit([]);
        }
    }

    fn load_all_instances_to_gpu(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        schedule: LoadToGPUSchedule,
    ) {
        if self.max_index > self.gpu_capacity {
            self.recreate_gpu_buffer(context);
        }

        context.queue.write_buffer(
            self.gpu_buffer(),
            0,
            bytemuck::cast_slice(self.occupied_buffer()),
        );

        if schedule == LoadToGPUSchedule::Immediately {
            context.queue.submit([]);
        }
    }

    fn take_buffer_resized(&mut self) -> bool {
        let was = self.buffer_resized;
        self.buffer_resized = false;
        was
    }

    #[inline]
    fn gpu_buffer(&self) -> &wgpu::Buffer {
        self.gpu_buffer
            .as_ref()
            .expect("Buffer has not been created")
    }

    fn ranges_iter(&self) -> InstancesRangesIter<ID> {
        InstancesRangesIter {
            removed_ids: self.removed_ids.clone(),
            current_id_index: 0,
            len: self.max_index,
            last_index: 0,
        }
    }

    fn contains(&self, id: ID) -> bool {
        self.ids.contains(&id)
    }

    fn ids(&self) -> &[ID] {
        &self.ids
    }

    fn occupied_buffer(&self) -> &[T] {
        &self.buffer_data[0..self.max_index]
    }

    fn full_buffer_data(&self) -> &[T] {
        &self.buffer_data
    }

    fn instance_buffer(&self, id: ID) -> &[T] {
        let idx = id.index();

        debug_assert!(idx < self.max_index, "Instance not found");
        debug_assert!(
            !self.removed_ids.contains(&id),
            "Instance marked as removed"
        );

        &self.buffer_data[idx..idx + 1]
    }
}

impl<ID: InstanceId + Copy + Clone + PartialEq, T: Default + Clone> PoolInstances<ID, T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer_data: vec![T::default(); capacity],
            removed_ids: Vec::with_capacity(capacity),
            ids: Vec::with_capacity(capacity),
            max_index: 0,
            id_factory: None,
            gpu_buffer: None,
            gpu_capacity: 0,
            buffer_resized: false,
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn insert(&mut self, data: T) -> ID {
        let id_factory = self
            .id_factory
            .expect("Instances has not been initizalized");

        let index = if self.removed_ids.is_empty() {
            let idx = self.max_index;
            self.max_index += 1;

            if idx >= self.buffer_data.len() {
                self.buffer_data
                    .resize(self.buffer_data.len() * 2, T::default());
            }

            idx
        } else {
            self.removed_ids.pop().unwrap().index()
        };

        let id = id_factory(index, &data);
        self.buffer_data[index] = data;

        self.ids.push(id);

        id
    }

    fn recreate_gpu_buffer(&mut self, context: &GraphicsContext<'_, '_>)
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        let gpu_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SVG Shader Instance Buffer"),
                contents: bytemuck::cast_slice(&self.buffer_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        self.gpu_buffer = Some(gpu_buffer);
        self.gpu_capacity = self.buffer_data.len();
        self.buffer_resized = true;
    }

    pub fn clear(&mut self) {
        self.max_index = 0;
        self.ids.clear();
        self.removed_ids.clear();
    }

    pub fn remove(&mut self, id: ID) {
        let index = self
            .ids
            .iter()
            .position(|other| *other == id)
            .expect("Id has not been found");
        self.ids.remove(index);
        self.removed_ids.push(id);
        self.removed_ids.sort_by_key(|a| a.index());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Range;

    // Mock implementation of InstanceId for testing
    #[derive(Copy, Clone, PartialEq, Debug)]
    struct TestId(usize);

    impl InstanceId for TestId {
        fn index(&self) -> usize {
            self.0
        }
    }

    fn create_test_instances() -> PoolInstances<TestId, i32> {
        PoolInstances {
            buffer_data: vec![0; 10], // Pre-allocate some space
            removed_ids: Vec::new(),
            ids: Vec::new(),
            max_index: 0,
            id_factory: Some(|idx, _| TestId(idx)),
            gpu_buffer: None,
            gpu_capacity: 10,
            buffer_resized: false,
        }
    }

    #[test]
    fn test_add_instance() {
        let mut instances = create_test_instances();

        let id1 = instances.insert(42);
        assert_eq!(id1.index(), 0);
        assert_eq!(instances.buffer_data[0], 42);
        assert_eq!(instances.max_index, 1);

        let id2 = instances.insert(24);
        assert_eq!(id2.index(), 1);
        assert_eq!(instances.buffer_data[1], 24);
        assert_eq!(instances.max_index, 2);
    }

    #[test]
    fn test_remove_instance() {
        let mut instances = create_test_instances();

        let id1 = instances.insert(42);
        let id2 = instances.insert(24);

        instances.remove(id1);
        assert!(!instances.ids().contains(&id1));
        assert!(instances.ids().contains(&id2));
        assert_eq!(instances.removed_ids.len(), 1);
        assert_eq!(instances.removed_ids[0], id1);
    }

    #[test]
    fn test_reuse_removed_index() {
        let mut instances = create_test_instances();

        let id1 = instances.insert(42);
        instances.remove(id1);

        let id2 = instances.insert(24);
        assert_eq!(id1.index(), id2.index());
        assert!(instances.removed_ids.is_empty());
    }

    #[test]
    fn test_ranges_iter() {
        let mut instances = create_test_instances();

        instances.insert(1);
        let id2 = instances.insert(2);
        let id3 = instances.insert(3);
        instances.insert(4);

        instances.remove(id2);
        instances.remove(id3);

        let ranges: Vec<Range<u32>> = instances.ranges_iter().collect();

        println!("{:?}", ranges);

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], (0..1));
        assert_eq!(ranges[1], (3..4));
    }

    #[test]
    fn test_ranges_empty_iter() {
        let instances = create_test_instances();
        let ranges: Vec<Range<u32>> = instances.ranges_iter().collect();

        assert_eq!(ranges.len(), 0);
    }

    #[test]
    fn test_ranges_iter_when_remove_last_item() {
        let mut instances = create_test_instances();

        instances.insert(1);
        let id2 = instances.insert(2);
        instances.insert(3);
        let id4 = instances.insert(4);

        instances.remove(id2);
        instances.remove(id4);

        let ranges: Vec<Range<u32>> = instances.ranges_iter().collect();

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], (0..1));
        assert_eq!(ranges[1], (2..3));
    }

    #[test]
    fn test_ranges_iter_when_remove_first_item() {
        let mut instances = create_test_instances();

        let id1 = instances.insert(1);
        instances.insert(2);
        instances.insert(3);
        instances.insert(4);

        instances.remove(id1);

        let ranges: Vec<Range<u32>> = instances.ranges_iter().collect();

        println!("RANGES: {:?}", ranges);

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (1..4));
    }

    #[test]
    fn test_ranges_iter_when_remove_first_consequent_items() {
        let mut instances = create_test_instances();

        let id1 = instances.insert(1);
        let id2 = instances.insert(2);
        instances.insert(3);
        instances.insert(4);

        instances.remove(id1);
        instances.remove(id2);

        let ranges: Vec<Range<u32>> = instances.ranges_iter().collect();

        println!("RANGES: {:?}", ranges);

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (2..4));
    }

    #[test]
    fn test_ranges_iter_when_remove_last_consequent_items() {
        let mut instances = create_test_instances();

        instances.insert(1);
        instances.insert(2);
        let id3 = instances.insert(3);
        let id4 = instances.insert(4);
        instances.insert(5);
        let id6 = instances.insert(6);

        instances.remove(id3);
        instances.remove(id4);
        instances.remove(id6);

        let ranges: Vec<Range<u32>> = instances.ranges_iter().collect();

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], (0..2));
        assert_eq!(ranges[1], (3..5));
    }

    #[test]
    fn test_ranges_without_removing_iter() {
        let mut instances = create_test_instances();

        instances.insert(1);
        instances.insert(2);
        instances.insert(3);
        instances.insert(4);

        let ranges: Vec<Range<u32>> = instances.ranges_iter().collect();

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (0..4));
    }

    #[test]
    fn test_ranges_with_all_removed() {
        let mut instances = create_test_instances();

        let id1 = instances.insert(1);
        let id2 = instances.insert(2);
        let id3 = instances.insert(3);
        let id4 = instances.insert(4);

        instances.remove(id1);
        instances.remove(id2);
        instances.remove(id3);
        instances.remove(id4);

        let ranges: Vec<Range<u32>> = instances.ranges_iter().collect();

        assert_eq!(ranges.len(), 0);
    }

    #[test]
    fn test_bump_instances_grow_on_overflow() {
        let mut instances = BumpInstances {
            buffer_data: vec![0i32; 2],
            ids: Vec::new(),
            max_index: 0,
            id_factory: Some(|idx, _| TestId(idx)),
            gpu_buffer: None,
            gpu_capacity: 0,
            buffer_resized: false,
        };

        instances.insert(1);
        instances.insert(2);
        instances.insert(3); // exceeds initial capacity of 2

        assert_eq!(instances.buffer_data.len(), 4); // grew 2x
        assert_eq!(instances.max_index, 3);
        assert_eq!(instances.buffer_data[2], 3);
    }

    #[test]
    fn test_pool_instances_grow_on_overflow() {
        let mut instances = PoolInstances {
            buffer_data: vec![0i32; 2],
            removed_ids: Vec::new(),
            ids: Vec::new(),
            max_index: 0,
            id_factory: Some(|idx, _| TestId(idx)),
            gpu_buffer: None,
            gpu_capacity: 0,
            buffer_resized: false,
        };

        instances.insert(1);
        instances.insert(2);
        instances.insert(3); // exceeds initial capacity of 2

        assert_eq!(instances.buffer_data.len(), 4); // grew 2x
        assert_eq!(instances.max_index, 3);
        assert_eq!(instances.buffer_data[2], 3);
    }

    #[test]
    fn test_pool_instances_slot_reuse_does_not_grow() {
        let mut instances = PoolInstances {
            buffer_data: vec![0i32; 2],
            removed_ids: Vec::new(),
            ids: Vec::new(),
            max_index: 0,
            id_factory: Some(|idx, _| TestId(idx)),
            gpu_buffer: None,
            gpu_capacity: 0,
            buffer_resized: false,
        };

        let id0 = instances.insert(1);
        instances.insert(2);
        instances.remove(id0);
        instances.insert(99); // reuses slot 0, no grow needed

        assert_eq!(instances.buffer_data.len(), 2); // no growth
        assert_eq!(instances.buffer_data[0], 99);
    }

    #[test]
    fn test_buffer_resized_starts_false() {
        let instances = BumpInstances::<TestId, i32>::new(4);
        assert!(!instances.buffer_resized);
    }

    #[test]
    fn test_take_buffer_resized_returns_false_without_gpu_recreate() {
        let mut instances = BumpInstances::<TestId, i32>::new(4);
        assert!(!instances.take_buffer_resized());
        assert!(!instances.take_buffer_resized()); // idempotent
    }
}
