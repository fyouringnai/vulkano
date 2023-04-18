// Copyright (c) 2016 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

// This is a modification of the triangle example, that demonstrates the basics of occlusion
// queries. Occlusion queries allow you to query whether, and sometimes how many, pixels pass the
// depth test in a range of draw calls.

use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
        RenderPassBeginInfo, SubpassContents,
    },
    device::{
        physical::PhysicalDeviceType, Device, DeviceCreateInfo, DeviceExtensions, QueueCreateInfo,
        QueueFlags,
    },
    format::Format,
    image::{view::ImageView, AttachmentImage, ImageAccess, ImageUsage, SwapchainImage},
    instance::{Instance, InstanceCreateInfo},
    memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator},
    pipeline::{
        graphics::{
            color_blend::ColorBlendState,
            depth_stencil::DepthStencilState,
            input_assembly::InputAssemblyState,
            multisample::MultisampleState,
            rasterization::RasterizationState,
            vertex_input::Vertex,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline,
    },
    query::{QueryControlFlags, QueryPool, QueryPoolCreateInfo, QueryResultFlags, QueryType},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    shader::PipelineShaderStageCreateInfo,
    swapchain::{
        acquire_next_image, AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError,
        SwapchainPresentInfo,
    },
    sync::{self, FlushError, GpuFuture},
    VulkanLibrary,
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

fn main() {
    let library = VulkanLibrary::new().unwrap();
    let required_extensions = vulkano_win::required_extensions(&library);
    let instance = Instance::new(
        library,
        InstanceCreateInfo {
            enabled_extensions: required_extensions,
            enumerate_portability: true,
            ..Default::default()
        },
    )
    .unwrap();

    let event_loop = EventLoop::new();
    let surface = WindowBuilder::new()
        .build_vk_surface(&event_loop, instance.clone())
        .unwrap();

    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        ..DeviceExtensions::empty()
    };
    let (physical_device, queue_family_index) = instance
        .enumerate_physical_devices()
        .unwrap()
        .filter(|p| p.supported_extensions().contains(&device_extensions))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.intersects(QueueFlags::GRAPHICS)
                        && p.surface_support(i as u32, &surface).unwrap_or(false)
                })
                .map(|i| (p, i as u32))
        })
        .min_by_key(|(p, _)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            PhysicalDeviceType::VirtualGpu => 2,
            PhysicalDeviceType::Cpu => 3,
            PhysicalDeviceType::Other => 4,
            _ => 5,
        })
        .unwrap();

    println!(
        "Using device: {} (type: {:?})",
        physical_device.properties().device_name,
        physical_device.properties().device_type,
    );

    let (device, mut queues) = Device::new(
        physical_device,
        DeviceCreateInfo {
            enabled_extensions: device_extensions,
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .unwrap();
    let queue = queues.next().unwrap();

    let (mut swapchain, images) = {
        let surface_capabilities = device
            .physical_device()
            .surface_capabilities(&surface, Default::default())
            .unwrap();
        let image_format = Some(
            device
                .physical_device()
                .surface_formats(&surface, Default::default())
                .unwrap()[0]
                .0,
        );
        let window = surface.object().unwrap().downcast_ref::<Window>().unwrap();

        Swapchain::new(
            device.clone(),
            surface.clone(),
            SwapchainCreateInfo {
                min_image_count: surface_capabilities.min_image_count,
                image_format,
                image_extent: window.inner_size().into(),
                image_usage: ImageUsage::COLOR_ATTACHMENT,
                composite_alpha: surface_capabilities
                    .supported_composite_alpha
                    .into_iter()
                    .next()
                    .unwrap(),
                ..Default::default()
            },
        )
        .unwrap()
    };

    let memory_allocator = StandardMemoryAllocator::new_default(device.clone());

    #[derive(BufferContents, Vertex)]
    #[repr(C)]
    struct Vertex {
        #[format(R32G32B32_SFLOAT)]
        position: [f32; 3],
        #[format(R32G32B32_SFLOAT)]
        color: [f32; 3],
    }

    let vertices = [
        // The first triangle (red) is the same one as in the triangle example.
        Vertex {
            position: [-0.5, -0.25, 0.5],
            color: [1.0, 0.0, 0.0],
        },
        Vertex {
            position: [0.0, 0.5, 0.5],
            color: [1.0, 0.0, 0.0],
        },
        Vertex {
            position: [0.25, -0.1, 0.5],
            color: [1.0, 0.0, 0.0],
        },
        // The second triangle (cyan) is the same shape and position as the first, but smaller, and
        // moved behind a bit. It should be completely occluded by the first triangle. (You can
        // lower its z value to put it in front.)
        Vertex {
            position: [-0.25, -0.125, 0.6],
            color: [0.0, 1.0, 1.0],
        },
        Vertex {
            position: [0.0, 0.25, 0.6],
            color: [0.0, 1.0, 1.0],
        },
        Vertex {
            position: [0.125, -0.05, 0.6],
            color: [0.0, 1.0, 1.0],
        },
        // The third triangle (green) is the same shape and size as the first, but moved to the
        // left and behind the second. It is partially occluded by the first two.
        Vertex {
            position: [-0.25, -0.25, 0.7],
            color: [0.0, 1.0, 0.0],
        },
        Vertex {
            position: [0.25, 0.5, 0.7],
            color: [0.0, 1.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.1, 0.7],
            color: [0.0, 1.0, 0.0],
        },
    ];
    let vertex_buffer = Buffer::from_iter(
        &memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::VERTEX_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Upload,
            ..Default::default()
        },
        vertices,
    )
    .unwrap();

    // Create three buffer slices, one for each triangle.
    let triangle1 = vertex_buffer.clone().slice(0..3);
    let triangle2 = vertex_buffer.clone().slice(3..6);
    let triangle3 = vertex_buffer.slice(6..9);

    // Create a query pool for occlusion queries, with 3 slots.
    let query_pool = QueryPool::new(
        device.clone(),
        QueryPoolCreateInfo {
            query_count: 3,
            ..QueryPoolCreateInfo::query_type(QueryType::Occlusion)
        },
    )
    .unwrap();

    // Create a buffer on the CPU to hold the results of the three queries. Query results are
    // always represented as either `u32` or `u64`. For occlusion queries, you always need one
    // element per query. You can ask for the number of elements needed at runtime by calling
    // `QueryType::result_len`. If you retrieve query results with `with_availability` enabled,
    // then this array needs to be 6 elements long instead of 3.
    let mut query_results = [0u32; 3];

    mod vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            src: r"
                #version 450

                layout(location = 0) in vec3 position;
                layout(location = 1) in vec3 color;

                layout(location = 0) out vec3 v_color;

                void main() {
                    v_color = color;
                    gl_Position = vec4(position, 1.0);
                }
            ",
        }
    }

    mod fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            src: r"
                #version 450

                layout(location = 0) in vec3 v_color;
                layout(location = 0) out vec4 f_color;

                void main() {
                    f_color = vec4(v_color, 1.0);
                }
            ",
        }
    }

    let render_pass = vulkano::single_pass_renderpass!(
        device.clone(),
        attachments: {
            color: {
                load: Clear,
                store: Store,
                format: swapchain.image_format(),
                samples: 1,
            },
            depth: {
                load: Clear,
                store: DontCare,
                format: Format::D16_UNORM,
                samples: 1,
            },
        },
        pass: {
            color: [color],
            depth_stencil: {depth},
        },
    )
    .unwrap();

    let vs = vs::load(device.clone())
        .unwrap()
        .entry_point("main")
        .unwrap();
    let fs = fs::load(device.clone())
        .unwrap()
        .entry_point("main")
        .unwrap();
    let subpass = Subpass::from(render_pass.clone(), 0).unwrap();
    let pipeline = GraphicsPipeline::start()
        .stages([
            PipelineShaderStageCreateInfo::entry_point(vs),
            PipelineShaderStageCreateInfo::entry_point(fs),
        ])
        .vertex_input_state(Vertex::per_vertex())
        .input_assembly_state(InputAssemblyState::default())
        .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
        .rasterization_state(RasterizationState::default())
        .multisample_state(MultisampleState::default())
        // Enable depth testing, which is needed for occlusion queries to make sense at all. If you
        // disable depth testing, every pixel is considered to pass the depth test, so every query
        // will return a nonzero result.
        .depth_stencil_state(DepthStencilState::simple_depth_test())
        .color_blend_state(ColorBlendState::new(subpass.num_color_attachments()))
        .render_pass(subpass)
        .build(device.clone())
        .unwrap();

    let mut viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [0.0, 0.0],
        depth_range: 0.0..1.0,
    };

    let command_buffer_allocator =
        StandardCommandBufferAllocator::new(device.clone(), Default::default());

    let mut framebuffers = window_size_dependent_setup(
        &images,
        render_pass.clone(),
        &mut viewport,
        &memory_allocator,
    );

    let mut recreate_swapchain = false;
    let mut previous_frame_end = Some(sync::now(device.clone()).boxed());

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => {
            *control_flow = ControlFlow::Exit;
        }
        Event::WindowEvent {
            event: WindowEvent::Resized(_),
            ..
        } => {
            recreate_swapchain = true;
        }
        Event::RedrawEventsCleared => {
            let window = surface.object().unwrap().downcast_ref::<Window>().unwrap();
            let dimensions = window.inner_size();
            if dimensions.width == 0 || dimensions.height == 0 {
                return;
            }

            previous_frame_end.as_mut().unwrap().cleanup_finished();

            if recreate_swapchain {
                let (new_swapchain, new_images) = match swapchain.recreate(SwapchainCreateInfo {
                    image_extent: dimensions.into(),
                    ..swapchain.create_info()
                }) {
                    Ok(r) => r,
                    Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                    Err(e) => panic!("failed to recreate swapchain: {e}"),
                };

                swapchain = new_swapchain;
                framebuffers = window_size_dependent_setup(
                    &new_images,
                    render_pass.clone(),
                    &mut viewport,
                    &memory_allocator,
                );
                recreate_swapchain = false;
            }

            let (image_index, suboptimal, acquire_future) =
                match acquire_next_image(swapchain.clone(), None) {
                    Ok(r) => r,
                    Err(AcquireError::OutOfDate) => {
                        recreate_swapchain = true;
                        return;
                    }
                    Err(e) => panic!("failed to acquire next image: {e}"),
                };

            if suboptimal {
                recreate_swapchain = true;
            }

            let mut builder = AutoCommandBufferBuilder::primary(
                &command_buffer_allocator,
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )
            .unwrap();

            // Beginning or resetting a query is unsafe for now.
            unsafe {
                builder
                    // A query must be reset before each use, including the first use. This must be
                    // done outside a render pass.
                    .reset_query_pool(query_pool.clone(), 0..3)
                    .unwrap()
                    .set_viewport(0, [viewport.clone()])
                    .bind_pipeline_graphics(pipeline.clone())
                    .begin_render_pass(
                        RenderPassBeginInfo {
                            clear_values: vec![Some([0.0, 0.0, 1.0, 1.0].into()), Some(1.0.into())],
                            ..RenderPassBeginInfo::framebuffer(
                                framebuffers[image_index as usize].clone(),
                            )
                        },
                        SubpassContents::Inline,
                    )
                    .unwrap()
                    // Begin query 0, then draw the red triangle. Enabling the
                    // `QueryControlFlags::PRECISE` flag would give exact numeric results. This
                    // needs the `occlusion_query_precise` feature to be enabled on the device.
                    .begin_query(
                        query_pool.clone(),
                        0,
                        QueryControlFlags::empty(),
                        // QueryControlFlags::PRECISE,
                    )
                    .unwrap()
                    .bind_vertex_buffers(0, triangle1.clone())
                    .draw(triangle1.len() as u32, 1, 0, 0)
                    .unwrap()
                    // End query 0.
                    .end_query(query_pool.clone(), 0)
                    .unwrap()
                    // Begin query 1 for the cyan triangle.
                    .begin_query(query_pool.clone(), 1, QueryControlFlags::empty())
                    .unwrap()
                    .bind_vertex_buffers(0, triangle2.clone())
                    .draw(triangle2.len() as u32, 1, 0, 0)
                    .unwrap()
                    .end_query(query_pool.clone(), 1)
                    .unwrap()
                    // Finally, query 2 for the green triangle.
                    .begin_query(query_pool.clone(), 2, QueryControlFlags::empty())
                    .unwrap()
                    .bind_vertex_buffers(0, triangle3.clone())
                    .draw(triangle3.len() as u32, 1, 0, 0)
                    .unwrap()
                    .end_query(query_pool.clone(), 2)
                    .unwrap()
                    .end_render_pass()
                    .unwrap();
            }

            let command_buffer = builder.build().unwrap();

            let future = previous_frame_end
                .take()
                .unwrap()
                .join(acquire_future)
                .then_execute(queue.clone(), command_buffer)
                .unwrap()
                .then_swapchain_present(
                    queue.clone(),
                    SwapchainPresentInfo::swapchain_image_index(swapchain.clone(), image_index),
                )
                .then_signal_fence_and_flush();

            match future {
                Ok(future) => {
                    previous_frame_end = Some(future.boxed());
                }
                Err(FlushError::OutOfDate) => {
                    recreate_swapchain = true;
                    previous_frame_end = Some(sync::now(device.clone()).boxed());
                }
                Err(e) => {
                    println!("failed to flush future: {e}");
                    previous_frame_end = Some(sync::now(device.clone()).boxed());
                }
            }

            // Retrieve the query results. This copies the results to a variable on the CPU. You
            // can also use the `copy_query_pool_results` function on a command buffer to write
            // results to a Vulkano buffer. This could then be used to influence draw operations
            // further down the line, either in the same frame or a future frame.
            #[rustfmt::skip]
            query_pool
                .queries_range(0..3)
                .unwrap()
                .get_results(
                    &mut query_results,
                    // Block the function call until the results are available.
                    // NOTE: If not all the queries have actually been executed, then this will 
                    // wait forever for something that never happens!
                    QueryResultFlags::WAIT

                    // Enable this flag to give partial results if available, instead of waiting
                    // for the full results.
                    // | QueryResultFlags::PARTIAL

                    // Blocking and waiting will ensure the results are always available after the 
                    // function returns.
                    //
                    // If you disable waiting, then this flag can be enabled to include the
                    // availability of each query's results. You need one extra element per query 
                    // in your `query_results` buffer for this. This element will be filled with a 
                    // zero/nonzero value indicating availability.
                    // | QueryResultFlags::WITH_AVAILABILITY
                )
                .unwrap();

            // If the `precise` bit was not enabled, then you're only guaranteed to get a boolean
            // result here: zero if all pixels were occluded, nonzero if only some were occluded.
            // Enabling `precise` will give the exact number of pixels.

            // Query 0 (red triangle) will always succeed, because the depth buffer starts empty
            // and will never occlude anything.
            assert_ne!(query_results[0], 0);

            // Query 1 (cyan triangle) will fail, because it's drawn completely behind the first.
            assert_eq!(query_results[1], 0);

            // Query 2 (green triangle) will succeed, because it's only partially occluded.
            assert_ne!(query_results[2], 0);
        }
        _ => (),
    });
}

fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage>],
    render_pass: Arc<RenderPass>,
    viewport: &mut Viewport,
    memory_allocator: &StandardMemoryAllocator,
) -> Vec<Arc<Framebuffer>> {
    let dimensions = images[0].dimensions().width_height();
    viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];

    let depth_attachment = ImageView::new_default(
        AttachmentImage::with_usage(
            memory_allocator,
            dimensions,
            Format::D16_UNORM,
            ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
        )
        .unwrap(),
    )
    .unwrap();

    images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone()).unwrap();
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view, depth_attachment.clone()],
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>()
}
