#![allow(dead_code)]

use easydrm::{gl, EasyDRM, MonitorContextCreationRequest};
use skia_safe::{self as skia, gpu, gpu::gl::FramebufferInfo};
use thiserror::Error;

// -----------------------------
// Errors
// -----------------------------

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("easydrm error: {0}")]
    EasyDrmError(#[from] easydrm::EasyDRMError),

    #[error("skia GL interface creation failed")]
    SkiaGlInterface,

    #[error("skia DirectContext creation failed")]
    SkiaDirectContext,

    #[error("skia surface creation failed")]
    SkiaSurface,
}

// -----------------------------
// Per-monitor render state
// -----------------------------

pub struct MonitorRenderState {
    pub gr: gpu::DirectContext,
    pub surface: skia::Surface,
    pub width: usize,
    pub height: usize,
    pub gl: gl::Gles2,
}

impl MonitorRenderState {
    fn new(req: &MonitorContextCreationRequest<'_>) -> Result<Self, RenderError> {
        let interface = gpu::gl::Interface::new_load_with(|s| (req.get_proc_address)(s))
            .ok_or(RenderError::SkiaGlInterface)?;

        let mut gr = gpu::direct_contexts::make_gl(interface, None).ok_or(RenderError::SkiaDirectContext)?;

        let surface = skia_surface_for_current_fbo(&mut gr, req.gl, req.width, req.height)?;

        Ok(Self {
            gr,
            surface,
            width: req.width,
            height: req.height,
            gl: req.gl.clone(),
        })
    }

    fn ensure_surface_size(&mut self, width: usize, height: usize) -> Result<(), RenderError> {
        if self.width == width && self.height == height {
            return Ok(());
        }
        self.width = width;
        self.height = height;
        self.surface = skia_surface_for_current_fbo(&mut self.gr, &self.gl, width, height)?;
        Ok(())
    }

    pub fn canvas(&mut self) -> &skia::Canvas {
        self.surface.canvas()
    }

    pub fn flush(&mut self) {
        self.gr.flush_and_submit();
    }
}

// -----------------------------
// Rendering layer
// -----------------------------

pub struct RenderingLayer {
    drm: EasyDRM<MonitorRenderState>,
}

impl RenderingLayer {
    pub fn init() -> Result<Self, RenderError> {
        let drm = EasyDRM::init(|req| {
            // O EasyDRM chama isto com o contexto do monitor já válido/current.
            MonitorRenderState::new(req).expect("MonitorRenderState::new failed")
        })?;

        Ok(Self { drm })
    }

    pub async fn run(mut self) -> Result<(), RenderError> {
        loop {
            // Mantém as surfaces a seguir ao tamanho real do monitor
            for mon in self.drm.monitors_mut() {
                if mon.can_render() && mon.make_current().is_ok() {
                    let mode = mon.active_mode();
                    let (w, h) = (mode.size().0 as usize, mode.size().1 as usize);
                    mon.context_mut().ensure_surface_size(w, h)?;

                    // Render stub (intencionalmente vazio por agora)
                    // Quando fores começar a desenhar:
                    let canvas = mon.context_mut().canvas();
                    canvas.clear(skia::Color::WHITE);
                    mon.context_mut().flush();
                }
            }

            self.drm.swap_buffers()?;
            self.drm.poll_events_async().await?;
        }
    }

    pub fn drm(&self) -> &EasyDRM<MonitorRenderState> {
        &self.drm
    }

    pub fn drm_mut(&mut self) -> &mut EasyDRM<MonitorRenderState> {
        &mut self.drm
    }
}

// -----------------------------
// Skia surface helper
// -----------------------------

fn skia_surface_for_current_fbo(
    gr: &mut gpu::DirectContext,
    gl: &gl::Gles2,
    width: usize,
    height: usize,
) -> Result<skia::Surface, RenderError> {
    let mut fbo: i32 = 0;
    unsafe {
        gl.GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fbo);
    }

    let fb_info = FramebufferInfo {
        fboid: fbo as u32,
        format: gpu::gl::Format::RGBA8.into(),
        protected: gpu::Protected::No,
    };

    let backend_rt = gpu::backend_render_targets::make_gl(
        (width as i32, height as i32),
        0, // samples
        8, // stencil
        fb_info,
    );

    gpu::surfaces::wrap_backend_render_target(
        gr,
        &backend_rt,
        gpu::SurfaceOrigin::BottomLeft,
        skia::ColorType::RGBA8888,
        None,
        None,
    )
    .ok_or(RenderError::SkiaSurface)
}
