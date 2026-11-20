use std::ffi::CStr;
use std::mem::size_of;
use std::ptr;

use ash::vk;

use crate::physics::{ContinuumWorld, GpuMaterial, GpuParticle};

pub const MAX_GPU_PARTICLES: usize = 196_608;
const HASH_BUCKETS: usize = 131_072;
const GRAV_CELLS: usize = 1_024;
const GRAV_SLOTS: usize = GRAV_CELLS * 2;
const WORKGROUP_SIZE: u32 = 256;

const SPV_CLEAR_GRID: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/clear_grid.comp.spv"));
const SPV_BUILD_GRID_EOS: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/build_grid_eos.comp.spv"));
const SPV_CONTINUUM_FORCES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/continuum_forces.comp.spv"));
const SPV_INTEGRATE_PLASTICITY: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/integrate_plasticity.comp.spv"));
const SPV_GRAV_CLEAR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/grav_clear.comp.spv"));
const SPV_GRAV_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/grav_bin.comp.spv"));
const SPV_GRAV_FORCE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/grav_force.comp.spv"));

#[repr(C)]
#[derive(Clone, Copy)]
struct PushConsts {
    num_particles: u32,
    step_index: u32,
    dt: f32,
    dx: f32,
    h: f32,
    cutoff: f32,
    grid_cell_size: f32,
    down_gravity: f32,
    grav_domain: [f32; 4],
}

struct GpuBuffer {
    buf: vk::Buffer,
    mem: vk::DeviceMemory,
    mapped: *mut u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MemKind {
    DeviceLocal,
    HostCachedStaging,
    HostCoherentUpload,
}

pub struct VulkanPhysicsEngine {
    _entry: ash::Entry,
    instance: ash::Instance,
    device: ash::Device,
    queue: vk::Queue,
    pub device_name: String,
    pub is_unified_mem: bool,

    particles_buf: GpuBuffer,
    staging_buf: GpuBuffer,
    materials_buf: GpuBuffer,
    scratch_buf: GpuBuffer,
    grid_head_buf: GpuBuffer,
    grid_next_buf: GpuBuffer,
    grav_grid_buf: GpuBuffer,

    desc_pool: vk::DescriptorPool,
    desc_layout: vk::DescriptorSetLayout,
    desc_set: vk::DescriptorSet,
    pipeline_layout: vk::PipelineLayout,

    pipe_clear_grid: vk::Pipeline,
    pipe_build_grid_eos: vk::Pipeline,
    pipe_continuum_forces: vk::Pipeline,
    pipe_integrate_plasticity: vk::Pipeline,
    pipe_grav_clear: vk::Pipeline,
    pipe_grav_bin: vk::Pipeline,
    pipe_grav_force: vk::Pipeline,

    cmd_pool: vk::CommandPool,
    cmd_buf: vk::CommandBuffer,
    fence: vk::Fence,

    synced_len: usize,
    prev_self_grav: bool,
}

unsafe impl Send for VulkanPhysicsEngine {}

fn bytes_to_spv_words(bytes: &[u8]) -> Result<Vec<u32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err("SPIR-V binary length is not a multiple of 4".to_string());
    }
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&c| u32::from_le_bytes(c))
        .collect())
}

impl VulkanPhysicsEngine {
    pub fn new() -> Result<Self, String> {
        unsafe {
            let entry =
                ash::Entry::load().map_err(|e| format!("Failed to load Vulkan loader: {e}"))?;

            let app_info = vk::ApplicationInfo::default()
                .api_version(vk::API_VERSION_1_2)
                .application_name(c"RustedSpaceSimulation")
                .engine_name(c"rssim-continuum-vulkan");

            let create_info = vk::InstanceCreateInfo::default().application_info(&app_info);
            let instance = entry
                .create_instance(&create_info, None)
                .map_err(|e| format!("vkCreateInstance failed: {e}"))?;

            let phys_devices = instance
                .enumerate_physical_devices()
                .map_err(|e| format!("enumerate_physical_devices failed: {e}"))?;
            if phys_devices.is_empty() {
                instance.destroy_instance(None);
                return Err("No Vulkan physical GPUs found".to_string());
            }

            let mut chosen: Option<(vk::PhysicalDevice, u32, i32, String)> = None;
            for pd in phys_devices {
                let props = instance.get_physical_device_properties(pd);
                let q_props = instance.get_physical_device_queue_family_properties(pd);
                let Some((q_idx, _)) = q_props
                    .iter()
                    .enumerate()
                    .find(|(_, q)| q.queue_flags.contains(vk::QueueFlags::COMPUTE))
                else {
                    continue;
                };
                let score = match props.device_type {
                    vk::PhysicalDeviceType::DISCRETE_GPU => 1000,
                    vk::PhysicalDeviceType::INTEGRATED_GPU => 600,
                    vk::PhysicalDeviceType::VIRTUAL_GPU => 250,
                    vk::PhysicalDeviceType::CPU => 10,
                    _ => 50,
                };
                let raw_name = CStr::from_ptr(props.device_name.as_ptr())
                    .to_string_lossy()
                    .into_owned();
                if chosen.as_ref().is_none_or(|c| score > c.2) {
                    chosen = Some((pd, q_idx as u32, score, raw_name));
                }
            }

            let Some((phys_dev, queue_family, _, device_name)) = chosen else {
                instance.destroy_instance(None);
                return Err("No Vulkan compute queue family found".to_string());
            };

            let priorities = [1.0f32];
            let q_info = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family)
                .queue_priorities(&priorities)];
            let dev_info = vk::DeviceCreateInfo::default().queue_create_infos(&q_info);
            let device = instance
                .create_device(phys_dev, &dev_info, None)
                .map_err(|e| {
                    instance.destroy_instance(None);
                    format!("vkCreateDevice failed: {e}")
                })?;

            let queue = device.get_device_queue(queue_family, 0);
            let mem_props = instance.get_physical_device_memory_properties(phys_dev);

            let p_bytes = (MAX_GPU_PARTICLES * size_of::<GpuParticle>()) as vk::DeviceSize;
            let (particles_buf, _) = Self::alloc_buffer(
                &device,
                &mem_props,
                p_bytes,
                vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_SRC
                    | vk::BufferUsageFlags::TRANSFER_DST,
                MemKind::DeviceLocal,
            )?;
            let (staging_buf, is_unified_mem) = Self::alloc_buffer(
                &device,
                &mem_props,
                p_bytes,
                vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::TRANSFER_DST,
                MemKind::HostCachedStaging,
            )?;
            let (materials_buf, _) = Self::alloc_buffer(
                &device,
                &mem_props,
                (34 * size_of::<GpuMaterial>()) as vk::DeviceSize,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                MemKind::HostCoherentUpload,
            )?;
            let (scratch_buf, _) = Self::alloc_buffer(
                &device,
                &mem_props,
                (MAX_GPU_PARTICLES * 64) as vk::DeviceSize,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                MemKind::DeviceLocal,
            )?;
            let (grid_head_buf, _) = Self::alloc_buffer(
                &device,
                &mem_props,
                (HASH_BUCKETS * size_of::<i32>()) as vk::DeviceSize,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                MemKind::DeviceLocal,
            )?;
            let (grid_next_buf, _) = Self::alloc_buffer(
                &device,
                &mem_props,
                (MAX_GPU_PARTICLES * size_of::<i32>()) as vk::DeviceSize,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                MemKind::DeviceLocal,
            )?;
            let (grav_grid_buf, _) = Self::alloc_buffer(
                &device,
                &mem_props,
                (GRAV_SLOTS * 16) as vk::DeviceSize,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                MemKind::DeviceLocal,
            )?;

            let mat_table = GpuMaterial::table();
            ptr::copy_nonoverlapping(
                mat_table.as_ptr() as *const u8,
                materials_buf.mapped,
                size_of::<[GpuMaterial; 34]>(),
            );

            let bindings: [vk::DescriptorSetLayoutBinding<'_>; 6] = std::array::from_fn(|b| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(b as u32)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
            });
            let desc_layout = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .map_err(|e| format!("create_descriptor_set_layout failed: {e}"))?;

            let pool_sizes = [vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 6,
            }];
            let desc_pool = device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(1)
                        .pool_sizes(&pool_sizes),
                    None,
                )
                .map_err(|e| format!("create_descriptor_pool failed: {e}"))?;

            let set_layouts = [desc_layout];
            let desc_set = device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(desc_pool)
                        .set_layouts(&set_layouts),
                )
                .map_err(|e| format!("allocate_descriptor_sets failed: {e}"))?[0];

            let buf_infos = [
                [vk::DescriptorBufferInfo {
                    buffer: particles_buf.buf,
                    offset: 0,
                    range: vk::WHOLE_SIZE,
                }],
                [vk::DescriptorBufferInfo {
                    buffer: materials_buf.buf,
                    offset: 0,
                    range: vk::WHOLE_SIZE,
                }],
                [vk::DescriptorBufferInfo {
                    buffer: scratch_buf.buf,
                    offset: 0,
                    range: vk::WHOLE_SIZE,
                }],
                [vk::DescriptorBufferInfo {
                    buffer: grid_head_buf.buf,
                    offset: 0,
                    range: vk::WHOLE_SIZE,
                }],
                [vk::DescriptorBufferInfo {
                    buffer: grid_next_buf.buf,
                    offset: 0,
                    range: vk::WHOLE_SIZE,
                }],
                [vk::DescriptorBufferInfo {
                    buffer: grav_grid_buf.buf,
                    offset: 0,
                    range: vk::WHOLE_SIZE,
                }],
            ];
            let writes: [vk::WriteDescriptorSet<'_>; 6] = std::array::from_fn(|b| {
                vk::WriteDescriptorSet::default()
                    .dst_set(desc_set)
                    .dst_binding(b as u32)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(&buf_infos[b])
            });
            device.update_descriptor_sets(&writes, &[]);

            let pc_ranges = [vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::COMPUTE,
                offset: 0,
                size: size_of::<PushConsts>() as u32,
            }];
            let pipeline_layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&set_layouts)
                        .push_constant_ranges(&pc_ranges),
                    None,
                )
                .map_err(|e| format!("create_pipeline_layout failed: {e}"))?;

            let pipe_clear_grid =
                Self::build_compute_pipeline(&device, pipeline_layout, SPV_CLEAR_GRID)?;
            let pipe_build_grid_eos =
                Self::build_compute_pipeline(&device, pipeline_layout, SPV_BUILD_GRID_EOS)?;
            let pipe_continuum_forces =
                Self::build_compute_pipeline(&device, pipeline_layout, SPV_CONTINUUM_FORCES)?;
            let pipe_integrate_plasticity =
                Self::build_compute_pipeline(&device, pipeline_layout, SPV_INTEGRATE_PLASTICITY)?;
            let pipe_grav_clear =
                Self::build_compute_pipeline(&device, pipeline_layout, SPV_GRAV_CLEAR)?;
            let pipe_grav_bin =
                Self::build_compute_pipeline(&device, pipeline_layout, SPV_GRAV_BIN)?;
            let pipe_grav_force =
                Self::build_compute_pipeline(&device, pipeline_layout, SPV_GRAV_FORCE)?;

            let cmd_pool = device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(queue_family)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .map_err(|e| format!("create_command_pool failed: {e}"))?;

            let cmd_buf = device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(cmd_pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .map_err(|e| format!("allocate_command_buffers failed: {e}"))?[0];

            let fence = device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .map_err(|e| format!("create_fence failed: {e}"))?;

            Ok(Self {
                _entry: entry,
                instance,
                device,
                queue,
                device_name,
                is_unified_mem,
                particles_buf,
                staging_buf,
                materials_buf,
                scratch_buf,
                grid_head_buf,
                grid_next_buf,
                grav_grid_buf,
                desc_pool,
                desc_layout,
                desc_set,
                pipeline_layout,
                pipe_clear_grid,
                pipe_build_grid_eos,
                pipe_continuum_forces,
                pipe_integrate_plasticity,
                pipe_grav_clear,
                pipe_grav_bin,
                pipe_grav_force,
                cmd_pool,
                cmd_buf,
                fence,
                synced_len: 0,
                prev_self_grav: false,
            })
        }
    }

    unsafe fn alloc_buffer(
        device: &ash::Device,
        mem_props: &vk::PhysicalDeviceMemoryProperties,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        kind: MemKind,
    ) -> Result<(GpuBuffer, bool), String> {
        let buf = device
            .create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
            .map_err(|e| format!("vkCreateBuffer failed: {e}"))?;

        let req = device.get_buffer_memory_requirements(buf);

        let find_mem = |flags: vk::MemoryPropertyFlags| -> Option<u32> {
            for i in 0..mem_props.memory_type_count {
                let supported = (req.memory_type_bits & (1 << i)) != 0;
                let has_flags = mem_props.memory_types[i as usize]
                    .property_flags
                    .contains(flags);
                if supported && has_flags {
                    return Some(i);
                }
            }
            None
        };

        let (mem_type_idx, cached_flag, map_it) = match kind {
            MemKind::HostCachedStaging => {
                // Prioritize HOST_CACHED so CPU readback runs at L3 RAM speed (~50+ GB/s instead of uncached WC!)
                if let Some(idx) = find_mem(
                    vk::MemoryPropertyFlags::HOST_VISIBLE
                        | vk::MemoryPropertyFlags::HOST_COHERENT
                        | vk::MemoryPropertyFlags::HOST_CACHED,
                ) {
                    (idx, true, true)
                } else if let Some(idx) = find_mem(
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                ) {
                    (idx, false, true)
                } else {
                    device.destroy_buffer(buf, None);
                    return Err("No HOST_VISIBLE | HOST_COHERENT staging memory".to_string());
                }
            }
            MemKind::HostCoherentUpload => {
                if let Some(idx) = find_mem(
                    vk::MemoryPropertyFlags::DEVICE_LOCAL
                        | vk::MemoryPropertyFlags::HOST_VISIBLE
                        | vk::MemoryPropertyFlags::HOST_COHERENT,
                ) {
                    (idx, true, true)
                } else if let Some(idx) = find_mem(
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                ) {
                    (idx, false, true)
                } else {
                    device.destroy_buffer(buf, None);
                    return Err("No HOST_VISIBLE upload memory".to_string());
                }
            }
            MemKind::DeviceLocal => {
                if let Some(idx) = find_mem(vk::MemoryPropertyFlags::DEVICE_LOCAL) {
                    (idx, false, false)
                } else if let Some(idx) = find_mem(vk::MemoryPropertyFlags::HOST_VISIBLE) {
                    (idx, false, false)
                } else {
                    device.destroy_buffer(buf, None);
                    return Err("No DEVICE_LOCAL memory".to_string());
                }
            }
        };

        let mem = device
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(req.size)
                    .memory_type_index(mem_type_idx),
                None,
            )
            .map_err(|e| {
                device.destroy_buffer(buf, None);
                format!("vkAllocateMemory failed: {e}")
            })?;

        device.bind_buffer_memory(buf, mem, 0).map_err(|e| {
            device.free_memory(mem, None);
            device.destroy_buffer(buf, None);
            format!("vkBindBufferMemory failed: {e}")
        })?;

        let mapped = if map_it {
            device
                .map_memory(mem, 0, size, vk::MemoryMapFlags::empty())
                .map_err(|e| format!("vkMapMemory failed: {e}"))? as *mut u8
        } else {
            ptr::null_mut()
        };

        Ok((GpuBuffer { buf, mem, mapped }, cached_flag))
    }

    unsafe fn build_compute_pipeline(
        device: &ash::Device,
        layout: vk::PipelineLayout,
        spv_bytes: &[u8],
    ) -> Result<vk::Pipeline, String> {
        let code = bytes_to_spv_words(spv_bytes)?;
        let shader_mod = device
            .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code), None)
            .map_err(|e| format!("create_shader_module failed: {e}"))?;

        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader_mod)
            .name(c"main");

        let info = [vk::ComputePipelineCreateInfo::default()
            .stage(stage)
            .layout(layout)];

        let res = device.create_compute_pipelines(vk::PipelineCache::null(), &info, None);
        device.destroy_shader_module(shader_mod, None);

        match res {
            Ok(pipes) => Ok(pipes[0]),
            Err((_, e)) => Err(format!("create_compute_pipelines failed: {e}")),
        }
    }

    /// Execute `substeps` full Continuum Elasto-Plastic + Reactive Shock steps in a single Vulkan command buffer!
    pub fn step_batch(&mut self, world: &mut ContinuumWorld, substeps: usize, dt: f32) {
        let n = world.len().min(MAX_GPU_PARTICLES);
        if n == 0 || substeps == 0 {
            return;
        }

        unsafe {
            let staging_slice =
                std::slice::from_raw_parts_mut(self.staging_buf.mapped as *mut GpuParticle, n);

            let self_grav_active = world.self_gravity && world.grav_g > 0.0;
            if self.prev_self_grav && !self_grav_active {
                world.gpu_dirty = true;
            }
            self.prev_self_grav = self_grav_active;

            let mut need_upload = false;
            if world.gpu_dirty || self.synced_len != n {
                world.export_gpu_particles(staging_slice);
                self.synced_len = n;
                need_upload = true;
            }
            let grav_domain = world.gravity_domain_params();

            let copy_bytes = (n * size_of::<GpuParticle>()) as vk::DeviceSize;
            let copy_regions = [vk::BufferCopy {
                src_offset: 0,
                dst_offset: 0,
                size: copy_bytes,
            }];

            let d = &self.device;
            d.reset_command_buffer(self.cmd_buf, vk::CommandBufferResetFlags::empty())
                .expect("reset_command_buffer");
            d.begin_command_buffer(
                self.cmd_buf,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .expect("begin_command_buffer");

            if need_upload {
                d.cmd_copy_buffer(
                    self.cmd_buf,
                    self.staging_buf.buf,
                    self.particles_buf.buf,
                    &copy_regions,
                );
                let upload_barrier = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
                d.cmd_pipeline_barrier(
                    self.cmd_buf,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &upload_barrier,
                    &[],
                    &[],
                );
            }

            d.cmd_bind_descriptor_sets(
                self.cmd_buf,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline_layout,
                0,
                &[self.desc_set],
                &[],
            );

            let shader_barrier = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];

            let groups_clear = (HASH_BUCKETS as u32).div_ceil(WORKGROUP_SIZE);
            let groups_grav_clear = (GRAV_SLOTS as u32).div_ceil(WORKGROUP_SIZE);
            let groups_pts = (n as u32).div_ceil(WORKGROUP_SIZE);

            for s in 0..substeps {
                let pc = PushConsts {
                    num_particles: n as u32,
                    step_index: (world.step_count + s as u64).min(u32::MAX as u64) as u32,
                    dt,
                    dx: world.dx,
                    h: world.h,
                    cutoff: world.cutoff,
                    grid_cell_size: world.grid_cell_size,
                    down_gravity: world.down_gravity,
                    grav_domain,
                };
                let pc_bytes = std::slice::from_raw_parts(
                    &pc as *const PushConsts as *const u8,
                    size_of::<PushConsts>(),
                );
                d.cmd_push_constants(
                    self.cmd_buf,
                    self.pipeline_layout,
                    vk::ShaderStageFlags::COMPUTE,
                    0,
                    pc_bytes,
                );

                d.cmd_bind_pipeline(
                    self.cmd_buf,
                    vk::PipelineBindPoint::COMPUTE,
                    self.pipe_clear_grid,
                );
                d.cmd_dispatch(self.cmd_buf, groups_clear, 1, 1);
                d.cmd_pipeline_barrier(
                    self.cmd_buf,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &shader_barrier,
                    &[],
                    &[],
                );

                d.cmd_bind_pipeline(
                    self.cmd_buf,
                    vk::PipelineBindPoint::COMPUTE,
                    self.pipe_build_grid_eos,
                );
                d.cmd_dispatch(self.cmd_buf, groups_pts, 1, 1);
                d.cmd_pipeline_barrier(
                    self.cmd_buf,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &shader_barrier,
                    &[],
                    &[],
                );

                // GPU-resident 32x32 LDS multipole self-gravity (updated every 8 substeps in VRAM!)
                if self_grav_active && (s == 0 || s % 8 == 0) {
                    d.cmd_bind_pipeline(
                        self.cmd_buf,
                        vk::PipelineBindPoint::COMPUTE,
                        self.pipe_grav_clear,
                    );
                    d.cmd_dispatch(self.cmd_buf, groups_grav_clear, 1, 1);
                    d.cmd_pipeline_barrier(
                        self.cmd_buf,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::DependencyFlags::empty(),
                        &shader_barrier,
                        &[],
                        &[],
                    );

                    d.cmd_bind_pipeline(
                        self.cmd_buf,
                        vk::PipelineBindPoint::COMPUTE,
                        self.pipe_grav_bin,
                    );
                    d.cmd_dispatch(self.cmd_buf, groups_pts, 1, 1);
                    d.cmd_pipeline_barrier(
                        self.cmd_buf,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::DependencyFlags::empty(),
                        &shader_barrier,
                        &[],
                        &[],
                    );

                    d.cmd_bind_pipeline(
                        self.cmd_buf,
                        vk::PipelineBindPoint::COMPUTE,
                        self.pipe_grav_force,
                    );
                    d.cmd_dispatch(self.cmd_buf, groups_pts, 1, 1);
                    d.cmd_pipeline_barrier(
                        self.cmd_buf,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::DependencyFlags::empty(),
                        &shader_barrier,
                        &[],
                        &[],
                    );
                }

                d.cmd_bind_pipeline(
                    self.cmd_buf,
                    vk::PipelineBindPoint::COMPUTE,
                    self.pipe_continuum_forces,
                );
                d.cmd_dispatch(self.cmd_buf, groups_pts, 1, 1);
                d.cmd_pipeline_barrier(
                    self.cmd_buf,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &shader_barrier,
                    &[],
                    &[],
                );

                d.cmd_bind_pipeline(
                    self.cmd_buf,
                    vk::PipelineBindPoint::COMPUTE,
                    self.pipe_integrate_plasticity,
                );
                d.cmd_dispatch(self.cmd_buf, groups_pts, 1, 1);

                if s + 1 < substeps {
                    d.cmd_pipeline_barrier(
                        self.cmd_buf,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::DependencyFlags::empty(),
                        &shader_barrier,
                        &[],
                        &[],
                    );
                } else {
                    let shader_to_xfer = [vk::MemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                        .dst_access_mask(vk::AccessFlags::TRANSFER_READ)];
                    d.cmd_pipeline_barrier(
                        self.cmd_buf,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::DependencyFlags::empty(),
                        &shader_to_xfer,
                        &[],
                        &[],
                    );
                }
            }

            // DMA burst-copy DEVICE_LOCAL particles -> HOST_CACHED L3 RAM for instant CPU readback!
            d.cmd_copy_buffer(
                self.cmd_buf,
                self.particles_buf.buf,
                self.staging_buf.buf,
                &copy_regions,
            );
            let xfer_to_host = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::HOST_READ)];
            d.cmd_pipeline_barrier(
                self.cmd_buf,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::HOST,
                vk::DependencyFlags::empty(),
                &xfer_to_host,
                &[],
                &[],
            );

            d.end_command_buffer(self.cmd_buf)
                .expect("end_command_buffer");

            d.reset_fences(&[self.fence]).expect("reset_fences");
            let cmd_bufs = [self.cmd_buf];
            let submits = [vk::SubmitInfo::default().command_buffers(&cmd_bufs)];
            d.queue_submit(self.queue, &submits, self.fence)
                .expect("vkQueueSubmit");
            d.wait_for_fences(&[self.fence], true, u64::MAX)
                .expect("vkWaitForFences");

            world.import_gpu_particles(staging_slice, substeps, dt);
        }
    }
}

impl Drop for VulkanPhysicsEngine {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_fence(self.fence, None);
            self.device.destroy_command_pool(self.cmd_pool, None);
            self.device.destroy_pipeline(self.pipe_clear_grid, None);
            self.device.destroy_pipeline(self.pipe_build_grid_eos, None);
            self.device
                .destroy_pipeline(self.pipe_continuum_forces, None);
            self.device
                .destroy_pipeline(self.pipe_integrate_plasticity, None);
            self.device.destroy_pipeline(self.pipe_grav_clear, None);
            self.device.destroy_pipeline(self.pipe_grav_bin, None);
            self.device.destroy_pipeline(self.pipe_grav_force, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_descriptor_pool(self.desc_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.desc_layout, None);

            for gb in [
                &self.particles_buf,
                &self.staging_buf,
                &self.materials_buf,
                &self.scratch_buf,
                &self.grid_head_buf,
                &self.grid_next_buf,
                &self.grav_grid_buf,
            ] {
                if !gb.mapped.is_null() {
                    self.device.unmap_memory(gb.mem);
                }
                self.device.destroy_buffer(gb.buf, None);
                self.device.free_memory(gb.mem, None);
            }

            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}
