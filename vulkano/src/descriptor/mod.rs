// Copyright (c) 2016 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

//! Provides a way for shaders to access the content of buffers and images, or read arbitrary data.
//!
//! If you except vertex attributes, there are two ways in Vulkan to pass data to a shader:
//!
//! - You can pass a very small amount of data (only a guaranteed 128 bytes) through the *push
//!   constants* mechanism. Push constants are the fastest and easiest way to pass data.
//! - You can make your shader read from a buffer or an image by binding it to a *descriptor*.
//!
//! Here is an example fragment shader in GLSL that uses both:
//!
//! ```ignore
//! #version 450
//!
//! // This is a descriptor that contains a texture.
//! layout(set = 0, binding = 0) uniform sampler2D u_texture;
//!
//! // This is a descriptor that contains a buffer.
//! layout(set = 0, binding = 1) uniform struct {
//!     int data[128];
//! } u_buffer;
//!
//! layout(push_constant) uniform PushConstants {
//!     // This is a push constant.
//!     float opacity;
//! } push_constants;
//!
//! layout(location = 0) in vec2 v_tex_coords;
//! layout(location = 0) out vec4 f_output;
//!
//! void main() {
//!     f_output.rgb = texture(u_texture, v_tex_coords).rgb;
//!     if (u_buffer.data[12] == 5) { f_output.rgb *= 2.0; }
//!     f_output.a = push_constants.opacity;
//! }
//! ```
//!
//! # Descriptors
//!
//! In order to read the content of a buffer or an image from a shader, that buffer or image
//! must be put in a *descriptor*. Each descriptor contains one buffer or one image alongside with
//! the way that it can be accessed. A descriptor can also be an array, in which case it contains
//! multiple buffers or images that all have the same layout.
//!
//! Descriptors are grouped in what is called *descriptor sets*. In Vulkan you don't bind
//! individual descriptors one by one, but you create then bind descriptor sets one by one. You are
//! therefore encouraged to put descriptors that are often used together in the same set.
//!
//! # Example
//!
//! > **Note**: This section describes the simple way to bind resources. There are more optimized
//! > ways.
//!
//! There are two steps to give access to a resource in a shader: creating the descriptor set, and
//! passing the descriptor sets when drawing.
//!
//! ## Creating a descriptor set
//!
//! ```ignore
//! // Creates a set that corresponds to the shader above.
//! // Note that the value `0` must match the set number.
//! let set0 = simple_descriptor_set!(&graphics_pipeline, 0, {
//!     u_texture: &my_buffer1,
//!     u_buffer: &my_buffer2,
//! });
//! ```
//!
//! ## Passing the descriptor set when drawing
//!
//! TODO: write
//!
//! # When drawing
//!
//! When you call a function that adds a draw command to a command buffer, one of the parameters
//! corresponds to the list of descriptor sets to use, and another parameter contains the push
//! constants. Vulkano will check that what you passed is compatible with the layout of the
//! compute or graphics pipeline.
//!
//! TODO: talk about perfs of changing sets

pub use self::descriptor_set::DescriptorSet;
pub use self::pipeline_layout::PipelineLayoutRef;

pub mod descriptor;
pub mod descriptor_set;
pub mod pipeline_layout;
