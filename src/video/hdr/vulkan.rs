//! Vulkan compute backend: packed 10-bit PQ/BT.2020 -> 8-bit BT.709 NV12.
//!
//! Capture buffers are imported as dma-bufs once; nothing is copied by the CPU.
//! Output buffers are dma-bufs as well (dma-heap when available, otherwise
//! exported from Vulkan) and are registered with the RKMPP encoder as inputs.
//! The loader is opened at runtime so images without Vulkan keep working.

use std::ffi::CStr;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::Path;
use std::time::Duration;

use ash::{ext, khr, vk, Device, Entry, Instance};

use super::colorimetry::pq_table;
use super::layout::{PackedLayout, HEIGHT, NV12_BYTES, WIDTH};
use super::stage::Converter;

const SHADER: &[u8] = include_bytes!("tonemap.spv");
const CONFIG_FLOATS: usize = 4 + 4096;
const FENCE_TIMEOUT: Duration = Duration::from_secs(5);
const PAGE: usize = 4096;
const DMA_BUF: vk::ExternalMemoryHandleTypeFlags = vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT;

/// Rendering choices, not mastering metadata: SDR white at 203 nits, 1000 nit
/// highlight roll-off.
#[derive(Debug, Clone, Copy)]
pub struct ToneParams {
    pub white_nits: f32,
    pub peak_nits: f32,
    pub dither: f32,
    pub full_range: bool,
}

impl Default for ToneParams {
    fn default() -> Self {
        Self {
            white_nits: 203.0,
            peak_nits: 1000.0,
            dither: 1.0,
            full_range: false,
        }
    }
}

fn vk_err(operation: &'static str) -> impl FnOnce(vk::Result) -> String {
    move |result| format!("{operation}: {result:?}")
}

struct Gpu {
    // Dropped in declaration order after `Drop::drop`: device, then instance,
    // then the loader that both were created from.
    device: Device,
    instance: Instance,
    _entry: Entry,
    physical: vk::PhysicalDevice,
    queue_family: u32,
    name: String,
}

impl Gpu {
    fn new() -> Result<Self, String> {
        let entry =
            unsafe { Entry::load() }.map_err(|e| format!("Vulkan loader unavailable: {e}"))?;
        let app = vk::ApplicationInfo::default()
            .application_name(c"one-kvm")
            .api_version(vk::API_VERSION_1_1);
        let instance = unsafe {
            entry.create_instance(
                &vk::InstanceCreateInfo::default().application_info(&app),
                None,
            )
        }
        .map_err(vk_err("vkCreateInstance"))?;
        match Self::open_device(&instance) {
            Ok((device, physical, queue_family, name)) => Ok(Self {
                device,
                instance,
                _entry: entry,
                physical,
                queue_family,
                name,
            }),
            Err(error) => {
                unsafe { instance.destroy_instance(None) };
                Err(error)
            }
        }
    }

    fn open_device(
        instance: &Instance,
    ) -> Result<(Device, vk::PhysicalDevice, u32, String), String> {
        let required: [&CStr; 3] = [
            khr::external_memory_fd::NAME,
            ext::external_memory_dma_buf::NAME,
            ext::queue_family_foreign::NAME,
        ];
        let devices = unsafe { instance.enumerate_physical_devices() }
            .map_err(vk_err("vkEnumeratePhysicalDevices"))?;
        for physical in devices {
            let props = unsafe { instance.get_physical_device_properties(physical) };
            if props.device_type == vk::PhysicalDeviceType::CPU {
                continue;
            }
            let name = unsafe { CStr::from_ptr(props.device_name.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            let available = unsafe { instance.enumerate_device_extension_properties(physical) }
                .unwrap_or_default();
            let has = |wanted: &CStr| {
                available
                    .iter()
                    .any(|ext| unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) } == wanted)
            };
            if !required.iter().all(|wanted| has(wanted)) {
                continue;
            }
            let families =
                unsafe { instance.get_physical_device_queue_family_properties(physical) };
            let Some(queue_family) = families
                .iter()
                .position(|family| family.queue_flags.contains(vk::QueueFlags::COMPUTE))
            else {
                continue;
            };
            let priorities = [1.0f32];
            let queue_info = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family as u32)
                .queue_priorities(&priorities)];
            let names: Vec<_> = required.iter().map(|name| name.as_ptr()).collect();
            let create = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_info)
                .enabled_extension_names(&names);
            let device = unsafe { instance.create_device(physical, &create, None) }
                .map_err(vk_err("vkCreateDevice"))?;
            return Ok((device, physical, queue_family as u32, name));
        }
        Err("no Vulkan GPU with dma-buf import support".into())
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

struct Slot {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
}

pub struct ToneMapper {
    // Drop::drop destroys these before `gpu` is dropped.
    slots: Vec<Slot>,
    sets_pool: vk::DescriptorPool,
    set_layout: vk::DescriptorSetLayout,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    command_pool: vk::CommandPool,
    fence: vk::Fence,
    queue: vk::Queue,
    command_buffers: Vec<vk::CommandBuffer>,
    inputs: Vec<(OwnedFd, usize)>,
    outputs: Vec<(OwnedFd, usize)>,
    in_flight: bool,
    gpu: Gpu,
}

impl ToneMapper {
    /// `inputs` are exported V4L2 capture buffers; `outputs` slots are created.
    pub fn new(
        layout: PackedLayout,
        params: ToneParams,
        inputs: Vec<(OwnedFd, usize)>,
        outputs: usize,
        dma_heap: Option<&Path>,
    ) -> Result<Self, String> {
        if inputs.is_empty() || outputs == 0 {
            return Err("tone mapper needs input and output buffers".into());
        }
        let gpu = Gpu::new()?;
        let queue = unsafe { gpu.device.get_device_queue(gpu.queue_family, 0) };
        let mut mapper = Self {
            slots: Vec::new(),
            sets_pool: vk::DescriptorPool::null(),
            set_layout: vk::DescriptorSetLayout::null(),
            pipeline_layout: vk::PipelineLayout::null(),
            pipeline: vk::Pipeline::null(),
            command_pool: vk::CommandPool::null(),
            fence: vk::Fence::null(),
            queue,
            command_buffers: Vec::new(),
            inputs,
            outputs: Vec::new(),
            in_flight: false,
            gpu,
        };
        // On error `mapper` is dropped, releasing whatever was created so far.
        mapper.build(layout, params, outputs, dma_heap)?;
        Ok(mapper)
    }

    pub fn device_name(&self) -> &str {
        &self.gpu.name
    }

    /// Duplicated output dma-bufs (and sizes) for registration with the encoder.
    pub fn output_buffers(&self) -> Result<Vec<(OwnedFd, usize)>, String> {
        self.outputs
            .iter()
            .map(|(fd, size)| {
                fd.try_clone()
                    .map(|fd| (fd, *size))
                    .map_err(|e| format!("dup output dma-buf: {e}"))
            })
            .collect()
    }

    fn build(
        &mut self,
        layout: PackedLayout,
        params: ToneParams,
        outputs: usize,
        dma_heap: Option<&Path>,
    ) -> Result<(), String> {
        let device = &self.gpu.device;
        let memory = unsafe {
            self.gpu
                .instance
                .get_physical_device_memory_properties(self.gpu.physical)
        };
        let external = khr::external_memory_fd::Device::new(&self.gpu.instance, device);

        // Configuration: [white, peak, dither, full_range, pq[4096]]. Every
        // slot is recorded in `self.slots` as soon as it exists so an error
        // anywhere below releases it in Drop.
        let config = self.create_config(&memory, params)?;
        let config_buffer = config.buffer;
        self.slots.push(config);

        let mut input_buffers = Vec::with_capacity(self.inputs.len());
        for index in 0..self.inputs.len() {
            let (raw, size) = (self.inputs[index].0.as_raw_fd(), self.inputs[index].1);
            let slot = self.import(&external, raw, size)?;
            input_buffers.push(slot.buffer);
            self.slots.push(slot);
        }
        let mut output_buffers = Vec::with_capacity(outputs);
        for _ in 0..outputs {
            let (slot, fd, size) = self.allocate_output(&external, &memory, dma_heap)?;
            output_buffers.push(slot.buffer);
            self.slots.push(slot);
            self.outputs.push((fd, size));
        }

        let bindings: Vec<_> = (0..3)
            .map(|binding| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(binding)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
            })
            .collect();
        self.set_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }
        .map_err(vk_err("vkCreateDescriptorSetLayout"))?;
        let push = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .size(20)];
        let layouts = [self.set_layout];
        self.pipeline_layout = unsafe {
            device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&layouts)
                    .push_constant_ranges(&push),
                None,
            )
        }
        .map_err(vk_err("vkCreatePipelineLayout"))?;

        let code: Vec<u32> = SHADER
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
            .collect();
        let module = unsafe {
            device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code), None)
        }
        .map_err(vk_err("vkCreateShaderModule"))?;
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(module)
            .name(c"main");
        let pipeline = unsafe {
            device.create_compute_pipelines(
                vk::PipelineCache::null(),
                &[vk::ComputePipelineCreateInfo::default()
                    .stage(stage)
                    .layout(self.pipeline_layout)],
                None,
            )
        };
        unsafe { device.destroy_shader_module(module, None) };
        self.pipeline = pipeline.map_err(|(_, e)| format!("vkCreateComputePipelines: {e:?}"))?[0];

        let combos = (input_buffers.len() * output_buffers.len()) as u32;
        let sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(3 * combos)];
        self.sets_pool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(combos)
                    .pool_sizes(&sizes),
                None,
            )
        }
        .map_err(vk_err("vkCreateDescriptorPool"))?;
        self.command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default().queue_family_index(self.gpu.queue_family),
                None,
            )
        }
        .map_err(vk_err("vkCreateCommandPool"))?;
        self.fence = unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None) }
            .map_err(vk_err("vkCreateFence"))?;
        self.command_buffers = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(self.command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(combos),
            )
        }
        .map_err(vk_err("vkAllocateCommandBuffers"))?;
        let set_layouts = vec![self.set_layout; combos as usize];
        let sets = unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(self.sets_pool)
                    .set_layouts(&set_layouts),
            )
        }
        .map_err(vk_err("vkAllocateDescriptorSets"))?;

        let words = (NV12_BYTES / 4) as u32;
        let luma_words = (WIDTH * HEIGHT / 4) as u32;
        let push_constants = [
            words,
            luma_words,
            layout.format.is_422() as u32,
            layout.stride,
            layout.chroma_offset(),
        ];
        // One work item converts a 4x2 tile and owns three output words.
        let groups = (luma_words / 2).div_ceil(256);

        for (i, &input) in input_buffers.iter().enumerate() {
            for (o, &output) in output_buffers.iter().enumerate() {
                let index = i * output_buffers.len() + o;
                let set = sets[index];
                let infos = [
                    [vk::DescriptorBufferInfo::default()
                        .buffer(input)
                        .range(vk::WHOLE_SIZE)],
                    [vk::DescriptorBufferInfo::default()
                        .buffer(config_buffer)
                        .range(vk::WHOLE_SIZE)],
                    [vk::DescriptorBufferInfo::default()
                        .buffer(output)
                        .range(vk::WHOLE_SIZE)],
                ];
                let writes: Vec<_> = infos
                    .iter()
                    .enumerate()
                    .map(|(binding, info)| {
                        vk::WriteDescriptorSet::default()
                            .dst_set(set)
                            .dst_binding(binding as u32)
                            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                            .buffer_info(info)
                    })
                    .collect();
                unsafe { device.update_descriptor_sets(&writes, &[]) };
                self.record(
                    self.command_buffers[index],
                    set,
                    input,
                    output,
                    &push_constants,
                    groups,
                )?;
            }
        }
        Ok(())
    }

    fn record(
        &self,
        cb: vk::CommandBuffer,
        set: vk::DescriptorSet,
        input: vk::Buffer,
        output: vk::Buffer,
        push: &[u32; 5],
        groups: u32,
    ) -> Result<(), String> {
        let device = &self.gpu.device;
        let family = self.gpu.queue_family;
        let transfer = |buffer, src, dst, from, to| {
            vk::BufferMemoryBarrier::default()
                .src_access_mask(src)
                .dst_access_mask(dst)
                .src_queue_family_index(from)
                .dst_queue_family_index(to)
                .buffer(buffer)
                .size(vk::WHOLE_SIZE)
        };
        unsafe {
            device
                .begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default())
                .map_err(vk_err("vkBeginCommandBuffer"))?;
            let acquire = [
                transfer(
                    input,
                    vk::AccessFlags::empty(),
                    vk::AccessFlags::SHADER_READ,
                    vk::QUEUE_FAMILY_FOREIGN_EXT,
                    family,
                ),
                transfer(
                    output,
                    vk::AccessFlags::empty(),
                    vk::AccessFlags::SHADER_WRITE,
                    vk::QUEUE_FAMILY_FOREIGN_EXT,
                    family,
                ),
            ];
            device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &acquire,
                &[],
            );
            device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, self.pipeline);
            device.cmd_bind_descriptor_sets(
                cb,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline_layout,
                0,
                &[set],
                &[],
            );
            device.cmd_push_constants(
                cb,
                self.pipeline_layout,
                vk::ShaderStageFlags::COMPUTE,
                0,
                bytemuck::bytes_of(push),
            );
            device.cmd_dispatch(cb, groups, 1, 1);
            let release = [
                transfer(
                    input,
                    vk::AccessFlags::SHADER_READ,
                    vk::AccessFlags::empty(),
                    family,
                    vk::QUEUE_FAMILY_FOREIGN_EXT,
                ),
                transfer(
                    output,
                    vk::AccessFlags::SHADER_WRITE,
                    vk::AccessFlags::empty(),
                    family,
                    vk::QUEUE_FAMILY_FOREIGN_EXT,
                ),
            ];
            device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &release,
                &[],
            );
            device
                .end_command_buffer(cb)
                .map_err(vk_err("vkEndCommandBuffer"))
        }
    }

    fn create_config(
        &self,
        memory: &vk::PhysicalDeviceMemoryProperties,
        params: ToneParams,
    ) -> Result<Slot, String> {
        let device = &self.gpu.device;
        let size = (CONFIG_FLOATS * 4) as u64;
        let buffer = unsafe {
            device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
        }
        .map_err(vk_err("vkCreateBuffer(config)"))?;
        let mut slot = Slot {
            buffer,
            memory: vk::DeviceMemory::null(),
        };
        let result = (|| {
            let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
            let host =
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            let index = (0..memory.memory_type_count)
                .find(|&i| {
                    requirements.memory_type_bits & (1 << i) != 0
                        && memory.memory_types[i as usize]
                            .property_flags
                            .contains(host)
                })
                .ok_or("no host-visible memory for tone-map config")?;
            slot.memory = unsafe {
                device.allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(requirements.size)
                        .memory_type_index(index),
                    None,
                )
            }
            .map_err(vk_err("vkAllocateMemory(config)"))?;
            unsafe { device.bind_buffer_memory(buffer, slot.memory, 0) }
                .map_err(vk_err("vkBindBufferMemory(config)"))?;
            let pointer =
                unsafe { device.map_memory(slot.memory, 0, size, vk::MemoryMapFlags::empty()) }
                    .map_err(vk_err("vkMapMemory(config)"))? as *mut f32;
            let mut values = vec![0f32; CONFIG_FLOATS];
            values[0] = params.white_nits;
            values[1] = params.peak_nits;
            values[2] = params.dither;
            values[3] = params.full_range as u32 as f32;
            values[4..].copy_from_slice(&pq_table());
            unsafe {
                std::ptr::copy_nonoverlapping(values.as_ptr(), pointer, CONFIG_FLOATS);
                device.unmap_memory(slot.memory);
            }
            Ok(())
        })();
        match result {
            Ok(()) => Ok(slot),
            Err(error) => {
                unsafe {
                    device.destroy_buffer(slot.buffer, None);
                    if slot.memory != vk::DeviceMemory::null() {
                        device.free_memory(slot.memory, None);
                    }
                }
                Err(error)
            }
        }
    }

    /// Import `fd` (not consumed) as a storage buffer of `bytes`.
    fn import(
        &self,
        external: &khr::external_memory_fd::Device,
        fd: RawFd,
        bytes: usize,
    ) -> Result<Slot, String> {
        let device = &self.gpu.device;
        let mut handle = vk::ExternalMemoryBufferCreateInfo::default().handle_types(DMA_BUF);
        let buffer = unsafe {
            device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(bytes as u64)
                    .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .push_next(&mut handle),
                None,
            )
        }
        .map_err(vk_err("vkCreateBuffer(import)"))?;
        let destroy = |error: String| {
            unsafe { device.destroy_buffer(buffer, None) };
            Err(error)
        };
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        if requirements.size as usize > bytes {
            return destroy(format!(
                "imported dma-buf of {bytes} bytes is smaller than the {} the GPU needs",
                requirements.size
            ));
        }
        let mut properties = vk::MemoryFdPropertiesKHR::default();
        if let Err(e) = unsafe { external.get_memory_fd_properties(DMA_BUF, fd, &mut properties) } {
            return destroy(format!("vkGetMemoryFdPropertiesKHR: {e:?}"));
        }
        let bits = requirements.memory_type_bits & properties.memory_type_bits;
        if bits == 0 {
            return destroy("dma-buf has no memory type usable by this GPU".into());
        }
        let owned = unsafe { libc::dup(fd) };
        if owned < 0 {
            return destroy(format!("dup dma-buf: {}", std::io::Error::last_os_error()));
        }
        let mut import = vk::ImportMemoryFdInfoKHR::default()
            .handle_type(DMA_BUF)
            .fd(owned);
        let memory = unsafe {
            device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(bytes as u64)
                    .memory_type_index(bits.trailing_zeros())
                    .push_next(&mut import),
                None,
            )
        };
        let memory = match memory {
            Ok(memory) => memory,
            Err(e) => {
                // The fd is consumed only by a successful import.
                unsafe { libc::close(owned) };
                return destroy(format!("vkAllocateMemory(import): {e:?}"));
            }
        };
        if let Err(e) = unsafe { device.bind_buffer_memory(buffer, memory, 0) } {
            unsafe { device.free_memory(memory, None) };
            return destroy(format!("vkBindBufferMemory(import): {e:?}"));
        }
        Ok(Slot { buffer, memory })
    }

    /// An NV12 destination: dma-heap when usable, else exported from Vulkan.
    fn allocate_output(
        &self,
        external: &khr::external_memory_fd::Device,
        memory: &vk::PhysicalDeviceMemoryProperties,
        dma_heap: Option<&Path>,
    ) -> Result<(Slot, OwnedFd, usize), String> {
        let rounded = NV12_BYTES.div_ceil(PAGE) * PAGE;
        if let Some(path) = dma_heap {
            match heap_alloc(path, rounded) {
                Ok(fd) => {
                    let slot = self.import(external, fd.as_raw_fd(), rounded)?;
                    return Ok((slot, fd, rounded));
                }
                Err(error) => tracing::warn!(
                    "HDR output dma-heap {} unavailable ({error}); exporting from Vulkan",
                    path.display()
                ),
            }
        }
        let device = &self.gpu.device;
        let mut handle = vk::ExternalMemoryBufferCreateInfo::default().handle_types(DMA_BUF);
        let buffer = unsafe {
            device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(NV12_BYTES as u64)
                    .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .push_next(&mut handle),
                None,
            )
        }
        .map_err(vk_err("vkCreateBuffer(output)"))?;
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        let destroy = |error: String| {
            unsafe { device.destroy_buffer(buffer, None) };
            Err(error)
        };
        let Some(index) =
            (0..memory.memory_type_count).find(|&i| requirements.memory_type_bits & (1 << i) != 0)
        else {
            return destroy("no memory type for the exported HDR output".into());
        };
        let mut export = vk::ExportMemoryAllocateInfo::default().handle_types(DMA_BUF);
        let allocated = unsafe {
            device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(index)
                    .push_next(&mut export),
                None,
            )
        };
        let allocated = match allocated {
            Ok(allocated) => allocated,
            Err(e) => return destroy(format!("vkAllocateMemory(output): {e:?}")),
        };
        let slot = Slot {
            buffer,
            memory: allocated,
        };
        let release = |error: String| {
            unsafe {
                device.destroy_buffer(buffer, None);
                device.free_memory(allocated, None);
            }
            Err(error)
        };
        if let Err(e) = unsafe { device.bind_buffer_memory(buffer, allocated, 0) } {
            return release(format!("vkBindBufferMemory(output): {e:?}"));
        }
        let fd = unsafe {
            external.get_memory_fd(
                &vk::MemoryGetFdInfoKHR::default()
                    .memory(allocated)
                    .handle_type(DMA_BUF),
            )
        };
        match fd {
            Ok(fd) => Ok((
                slot,
                unsafe { OwnedFd::from_raw_fd(fd) },
                requirements.size as usize,
            )),
            Err(e) => release(format!("vkGetMemoryFdKHR: {e:?}")),
        }
    }
}

impl Converter for ToneMapper {
    fn submit(&mut self, input: usize, output: usize) -> Result<(), String> {
        if self.in_flight {
            return Err("tone-map submit while a conversion is in flight".into());
        }
        if input >= self.inputs.len() || output >= self.outputs.len() {
            return Err("tone-map buffer index out of range".into());
        }
        let device = &self.gpu.device;
        let buffers = [self.command_buffers[input * self.outputs.len() + output]];
        unsafe {
            device
                .reset_fences(&[self.fence])
                .map_err(vk_err("vkResetFences"))?;
            device
                .queue_submit(
                    self.queue,
                    &[vk::SubmitInfo::default().command_buffers(&buffers)],
                    self.fence,
                )
                .map_err(vk_err("vkQueueSubmit"))?;
        }
        self.in_flight = true;
        Ok(())
    }

    fn wait(&mut self) -> Result<(), String> {
        if !self.in_flight {
            return Ok(());
        }
        let result = unsafe {
            self.gpu
                .device
                .wait_for_fences(&[self.fence], true, FENCE_TIMEOUT.as_nanos() as u64)
        };
        match result {
            Ok(()) => {
                self.in_flight = false;
                Ok(())
            }
            Err(e) => Err(format!("vkWaitForFences: {e:?}")),
        }
    }
}

impl Drop for ToneMapper {
    fn drop(&mut self) {
        let device = &self.gpu.device;
        unsafe {
            let _ = device.device_wait_idle();
            if self.fence != vk::Fence::null() {
                device.destroy_fence(self.fence, None);
            }
            if self.command_pool != vk::CommandPool::null() {
                device.destroy_command_pool(self.command_pool, None);
            }
            if self.pipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.pipeline, None);
            }
            if self.pipeline_layout != vk::PipelineLayout::null() {
                device.destroy_pipeline_layout(self.pipeline_layout, None);
            }
            if self.sets_pool != vk::DescriptorPool::null() {
                device.destroy_descriptor_pool(self.sets_pool, None);
            }
            if self.set_layout != vk::DescriptorSetLayout::null() {
                device.destroy_descriptor_set_layout(self.set_layout, None);
            }
            for slot in self.slots.drain(..) {
                device.destroy_buffer(slot.buffer, None);
                device.free_memory(slot.memory, None);
            }
        }
    }
}

#[repr(C)]
struct DmaHeapAllocationData {
    len: u64,
    fd: u32,
    fd_flags: u32,
    heap_flags: u64,
}

/// `_IOWR('H', 0, struct dma_heap_allocation_data)`
const DMA_HEAP_IOCTL_ALLOC: libc::c_ulong = 0xC018_4800;

fn heap_alloc(path: &Path, len: usize) -> Result<OwnedFd, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let heap = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| e.to_string())?;
    let mut data = DmaHeapAllocationData {
        len: len as u64,
        fd: 0,
        fd_flags: (libc::O_RDWR | libc::O_CLOEXEC) as u32,
        heap_flags: 0,
    };
    let rc = unsafe { libc::ioctl(heap.as_raw_fd(), DMA_HEAP_IOCTL_ALLOC, &mut data) };
    if rc < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(data.fd as RawFd) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_mapper_can_move_to_the_worker_thread() {
        fn assert_send<T: Send>() {}
        assert_send::<ToneMapper>();
    }

    #[test]
    fn shader_is_spirv_and_word_aligned() {
        assert_eq!(SHADER.len() % 4, 0);
        assert_eq!(&SHADER[..4], &0x0723_0203u32.to_le_bytes());
    }

    #[test]
    fn dma_heap_ioctl_number_matches_linux_uapi() {
        // _IOWR(type 'H', nr 0, 24-byte struct)
        let encoded = (3u64 << 30) | (24 << 16) | ((b'H' as u64) << 8);
        assert_eq!(DMA_HEAP_IOCTL_ALLOC as u64, encoded);
        assert_eq!(std::mem::size_of::<DmaHeapAllocationData>(), 24);
    }

    #[test]
    fn default_parameters_match_the_validated_rendering() {
        let params = ToneParams::default();
        assert_eq!(
            (params.white_nits, params.peak_nits, params.dither),
            (203.0, 1000.0, 1.0)
        );
        assert!(!params.full_range);
    }
}
