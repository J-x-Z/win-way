//! Hardware-Accelerated OpenGL Renderer for Wayland Surfaces
//!
//! This module handles GPU-based rendering of Wayland surface content
//! using OpenGL via glutin + glow.
//!
//! Design: Uses a simplified approach that creates GL context directly
//! from the winit window, avoiding glutin-winit's event loop conflicts.

use std::ffi::CString;
use std::num::NonZeroU32;
use std::sync::Arc;

use glow::HasContext;
use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, NotCurrentContext, NotCurrentGlContext, PossiblyCurrentContext, Version};
use glutin::display::{Display, DisplayApiPreference, GlDisplay};
use glutin::surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawWindowHandle};
use winit::window::Window;
use log::{info, debug, error, warn};

use crate::error::{Result, WinWayError};

/// OpenGL Renderer with hardware acceleration
pub struct Renderer {
    gl: Option<Arc<glow::Context>>,
    context: Option<PossiblyCurrentContext>,
    surface: Option<Surface<WindowSurface>>,
    gl_config: Option<Config>,
    program: Option<glow::Program>,
    vao: Option<glow::VertexArray>,
    texture: Option<glow::Texture>,
    texture_width: u32,
    texture_height: u32,
    initialized: bool,
    clear_color: [f32; 4],
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            gl: None,
            context: None,
            surface: None,
            gl_config: None,
            program: None,
            vao: None,
            texture: None,
            texture_width: 0,
            texture_height: 0,
            initialized: false,
            clear_color: [0.08, 0.08, 0.12, 1.0], // Dark blue-gray
        }
    }

    /// Initialize OpenGL context for the given window
    pub fn init(&mut self, window: &Window) -> std::result::Result<(), String> {
        info!("🎨 Initializing hardware-accelerated OpenGL renderer...");

        // Get handles
        let display_handle = window.display_handle()
            .map_err(|e| format!("Failed to get display handle: {}", e))?;
        let window_handle = window.window_handle()
            .map_err(|e| format!("Failed to get window handle: {}", e))?;

        // Create GL display using raw handles
        // On Windows, prefer WGL (native OpenGL)
        #[cfg(windows)]
        let preference = DisplayApiPreference::Wgl(Some(window_handle.as_raw()));
        #[cfg(not(windows))]
        let preference = DisplayApiPreference::Egl;

        let gl_display = unsafe {
            Display::new(display_handle.as_raw(), preference)
                .map_err(|e| format!("Failed to create GL display: {:?}", e))?
        };

        // Find suitable config
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .prefer_hardware_accelerated(Some(true))
            .build();

        let config = unsafe {
            gl_display
                .find_configs(template)
                .map_err(|e| format!("Failed to find configs: {:?}", e))?
                .reduce(|accum, config| {
                    // Prefer hardware-accelerated configs
                    if config.hardware_accelerated() && !accum.hardware_accelerated() {
                        config
                    } else if config.num_samples() > accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .ok_or_else(|| "No suitable GL config found".to_string())?
        };

        info!("✅ GL Config: {} samples, hw_accel={}", 
              config.num_samples(), 
              config.hardware_accelerated());

        // Create context attributes (try OpenGL 3.3 Core first)
        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(Some(window_handle.as_raw()));

        // Fallback attributes
        let fallback_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(None))
            .build(Some(window_handle.as_raw()));

        // Create the GL context
        let not_current_context = unsafe {
            gl_display
                .create_context(&config, &context_attributes)
                .or_else(|_| gl_display.create_context(&config, &fallback_attributes))
                .map_err(|e| format!("Failed to create GL context: {:?}", e))?
        };

        // Get window size for surface
        let size = window.inner_size();
        let (width, height) = (
            NonZeroU32::new(size.width.max(1)).unwrap(),
            NonZeroU32::new(size.height.max(1)).unwrap(),
        );

        // Create surface attributes
        let surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            window_handle.as_raw(),
            width,
            height,
        );

        // Create the GL surface
        let surface = unsafe {
            gl_display
                .create_window_surface(&config, &surface_attributes)
                .map_err(|e| format!("Failed to create surface: {:?}", e))?
        };

        // Make context current
        let context = not_current_context
            .make_current(&surface)
            .map_err(|e| format!("Failed to make context current: {:?}", e))?;

        // Try to enable VSync (non-blocking)
        match surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::new(1).unwrap())) {
            Ok(_) => info!("✅ VSync enabled"),
            Err(e) => warn!("⚠️ VSync not available: {:?}", e),
        }

        // Load GL functions
        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                gl_display.get_proc_address(s) as *const _
            })
        };
        let gl = Arc::new(gl);

        // Print GL info
        unsafe {
            let version = gl.get_parameter_string(glow::VERSION);
            let renderer = gl.get_parameter_string(glow::RENDERER);
            info!("📺 OpenGL: {}", version);
            info!("🖥️  GPU: {}", renderer);
        }

        // Initialize GL state
        unsafe {
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.clear_color(
                self.clear_color[0],
                self.clear_color[1],
                self.clear_color[2],
                self.clear_color[3],
            );
        }

        // Create shader program and VAO for texture rendering
        let (program, vao) = self.create_shader_program(&gl)?;

        self.gl = Some(gl);
        self.context = Some(context);
        self.surface = Some(surface);
        self.gl_config = Some(config);
        self.program = Some(program);
        self.vao = Some(vao);
        self.initialized = true;

        info!("✅ Hardware-accelerated OpenGL renderer ready!");
        Ok(())
    }

    /// Create shader program for texture rendering
    fn create_shader_program(&self, gl: &glow::Context) -> std::result::Result<(glow::Program, glow::VertexArray), String> {
        const VERTEX_SHADER_SOURCE: &str = r#"
            #version 330 core
            layout (location = 0) in vec2 aPos;
            layout (location = 1) in vec2 aTexCoord;
            out vec2 TexCoord;
            void main() {
                gl_Position = vec4(aPos, 0.0, 1.0);
                TexCoord = aTexCoord;
            }
        "#;

        const FRAGMENT_SHADER_SOURCE: &str = r#"
            #version 330 core
            in vec2 TexCoord;
            out vec4 FragColor;
            uniform sampler2D uTexture;
            uniform bool uHasTexture;
            uniform float uTime;
            void main() {
                if (uHasTexture) {
                    FragColor = texture(uTexture, TexCoord);
                } else {
                    // Animated gradient when no texture (visual feedback)
                    float t = uTime * 0.5;
                    vec3 col = vec3(
                        0.1 + 0.05 * sin(t + TexCoord.x * 3.0),
                        0.1 + 0.05 * sin(t + TexCoord.y * 3.0 + 1.0),
                        0.15 + 0.1 * sin(t + 2.0)
                    );
                    FragColor = vec4(col, 1.0);
                }
            }
        "#;

        unsafe {
            // Create and compile vertex shader
            let vs = gl.create_shader(glow::VERTEX_SHADER)
                .map_err(|e| format!("Failed to create vertex shader: {}", e))?;
            gl.shader_source(vs, VERTEX_SHADER_SOURCE);
            gl.compile_shader(vs);
            if !gl.get_shader_compile_status(vs) {
                return Err(format!("Vertex shader error: {}", gl.get_shader_info_log(vs)));
            }

            // Create and compile fragment shader
            let fs = gl.create_shader(glow::FRAGMENT_SHADER)
                .map_err(|e| format!("Failed to create fragment shader: {}", e))?;
            gl.shader_source(fs, FRAGMENT_SHADER_SOURCE);
            gl.compile_shader(fs);
            if !gl.get_shader_compile_status(fs) {
                return Err(format!("Fragment shader error: {}", gl.get_shader_info_log(fs)));
            }

            // Create program and link
            let program = gl.create_program()
                .map_err(|e| format!("Failed to create program: {}", e))?;
            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(format!("Program link error: {}", gl.get_program_info_log(program)));
            }

            // Cleanup shaders
            gl.delete_shader(vs);
            gl.delete_shader(fs);

            // Create VAO and VBO for fullscreen quad
            let vao = gl.create_vertex_array()
                .map_err(|e| format!("Failed to create VAO: {}", e))?;
            let vbo = gl.create_buffer()
                .map_err(|e| format!("Failed to create VBO: {}", e))?;

            // Fullscreen quad vertices: position (x, y) + texcoord (s, t)
            #[rustfmt::skip]
            let vertices: [f32; 24] = [
                // pos      // tex
                -1.0, -1.0, 0.0, 1.0,  // bottom-left
                 1.0, -1.0, 1.0, 1.0,  // bottom-right
                 1.0,  1.0, 1.0, 0.0,  // top-right
                -1.0, -1.0, 0.0, 1.0,  // bottom-left
                 1.0,  1.0, 1.0, 0.0,  // top-right
                -1.0,  1.0, 0.0, 0.0,  // top-left
            ];

            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            
            // Convert to bytes
            let vertex_bytes: &[u8] = std::slice::from_raw_parts(
                vertices.as_ptr() as *const u8,
                vertices.len() * std::mem::size_of::<f32>(),
            );
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertex_bytes, glow::STATIC_DRAW);

            // Position attribute
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
            gl.enable_vertex_attrib_array(0);

            // Texcoord attribute
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
            gl.enable_vertex_attrib_array(1);

            gl.bind_vertex_array(None);

            Ok((program, vao))
        }
    }

    /// Update texture from raw pixel data (RGBA)
    pub fn update_surface(&mut self, _surface_id: u32, data: &[u8], width: u32, height: u32) {
        let Some(gl) = &self.gl else { return };

        debug!("📺 Updating surface: {}x{} ({} bytes)", width, height, data.len());

        unsafe {
            // Create texture if needed or size changed
            if self.texture.is_none() || self.texture_width != width || self.texture_height != height {
                // Delete old texture
                if let Some(old_tex) = self.texture.take() {
                    gl.delete_texture(old_tex);
                }

                // Create new texture
                let texture = gl.create_texture().expect("Failed to create texture");
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);

                self.texture = Some(texture);
                self.texture_width = width;
                self.texture_height = height;
            }

            // Upload texture data
            if let Some(texture) = &self.texture {
                gl.bind_texture(glow::TEXTURE_2D, Some(*texture));
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    width as i32,
                    height as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(data)),
                );
            }
        }
    }

    /// Render a frame
    pub fn render(&self, window_width: u32, window_height: u32, time: f32) {
        let Some(gl) = &self.gl else { return };
        let Some(context) = &self.context else { return };
        let Some(surface) = &self.surface else { return };
        let Some(program) = &self.program else { return };
        let Some(vao) = &self.vao else { return };

        unsafe {
            gl.viewport(0, 0, window_width as i32, window_height as i32);
            gl.clear(glow::COLOR_BUFFER_BIT);

            gl.use_program(Some(*program));
            gl.bind_vertex_array(Some(*vao));

            // Set uniforms
            let has_texture = self.texture.is_some();
            if let Some(loc) = gl.get_uniform_location(*program, "uHasTexture") {
                gl.uniform_1_i32(Some(&loc), has_texture as i32);
            }
            if let Some(loc) = gl.get_uniform_location(*program, "uTime") {
                gl.uniform_1_f32(Some(&loc), time);
            }

            if let Some(texture) = &self.texture {
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, Some(*texture));
                if let Some(loc) = gl.get_uniform_location(*program, "uTexture") {
                    gl.uniform_1_i32(Some(&loc), 0);
                }
            }

            // Draw fullscreen quad
            gl.draw_arrays(glow::TRIANGLES, 0, 6);

            gl.bind_vertex_array(None);
        }

        // Swap buffers (VSync)
        if let Err(e) = surface.swap_buffers(context) {
            error!("Failed to swap buffers: {:?}", e);
        }
    }

    /// Resize the surface
    pub fn resize(&mut self, width: u32, height: u32) {
        let Some(context) = &self.context else { return };
        let Some(surface) = &self.surface else { return };

        if width > 0 && height > 0 {
            surface.resize(
                context,
                NonZeroU32::new(width).unwrap(),
                NonZeroU32::new(height).unwrap(),
            );
            debug!("Resized surface to {}x{}", width, height);
        }
    }

    /// Check if renderer is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Clean up resources
    pub fn cleanup(&mut self) {
        if let Some(gl) = &self.gl {
            unsafe {
                if let Some(texture) = self.texture.take() {
                    gl.delete_texture(texture);
                }
                if let Some(vao) = self.vao.take() {
                    gl.delete_vertex_array(vao);
                }
                if let Some(program) = self.program.take() {
                    gl.delete_program(program);
                }
            }
        }
        self.gl = None;
        self.context = None;
        self.surface = None;
        self.initialized = false;
        info!("🧹 Renderer cleaned up");
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.cleanup();
    }
}
