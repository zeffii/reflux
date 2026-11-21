//! Creating the wgpu device, with or without a window.

pub struct Gpu {
    #[allow(dead_code)] // kept alive alongside the surfaces it created
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    /// `compatible_surface` is the window's surface when there is one; headless
    /// rendering (screenshots) passes `None`.
    pub fn new(instance: wgpu::Instance, compatible_surface: Option<&wgpu::Surface<'_>>) -> Gpu {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .expect("no GPU adapter found");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("reflux device"),
            ..Default::default()
        }))
        .expect("could not open the GPU device");

        Gpu {
            instance,
            adapter,
            device,
            queue,
        }
    }

    pub fn describe(&self) -> String {
        let info = self.adapter.get_info();
        format!("{} ({:?})", info.name, info.backend)
    }
}
