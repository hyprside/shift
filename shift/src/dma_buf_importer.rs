use thiserror::Error;
use tracing::{debug, error};

use easydrm::gl;
use tab_protocol::FramebufferLinkPayload;

use crate::egl;
use crate::opengl::TextureBindGuard;

#[derive(Debug, Error)]
pub enum ExternalTextureError {
	#[error("EGL display is not initialized")]
	EglDisplayNotInitialized,

	#[error("Failed to create EGLImage from DMA-BUF (EGL error: {0:#06x})")]
	EglImageCreationFailed(i32),

	#[error("OpenGL texture creation failed")]
	GlTextureFailed,

	#[error("Invalid DMA-BUF fd")]
	InvalidFd,
}

pub struct ExternalTexture {
	pub gl: gl::Gles2,
	pub egl: egl::Egl,
	pub texture: u32,
	pub image: egl::types::EGLImageKHR,
	pub fd: std::os::fd::RawFd,
	// FIXME: width/height could be exposed/used for viewport setup; currently unused.
	pub width: i32,
	pub height: i32,
}

impl ExternalTexture {
	/// Import a DMA-BUF using a FramebufferLinkPayload + StructGenerator GL/EGL bindings
	pub unsafe fn import(
		gl: &gl::Gles2,
		egl: &egl::Egl,
		fd: std::os::fd::RawFd,
		payload: &FramebufferLinkPayload,
	) -> Result<Self, ExternalTextureError> {
		if fd < 0 {
			error!(fd, "Invalid DMA-BUF FD");
			return Err(ExternalTextureError::InvalidFd);
		}

		let display = egl.GetCurrentDisplay();
		if display == egl::NO_DISPLAY {
			error!("EGL display is not initialized");
			return Err(ExternalTextureError::EglDisplayNotInitialized);
		}

		debug!(?payload, fd, "Importing DMA-BUF as EGLImage");

		let attribs = [
			egl::LINUX_DRM_FOURCC_EXT as i32,
			payload.fourcc,
			egl::DMA_BUF_PLANE0_FD_EXT as i32,
			fd,
			egl::DMA_BUF_PLANE0_OFFSET_EXT as i32,
			payload.offset,
			egl::DMA_BUF_PLANE0_PITCH_EXT as i32,
			payload.stride,
			egl::WIDTH as i32,
			payload.width,
			egl::HEIGHT as i32,
			payload.height,
			egl::NONE as i32,
		];

		let image = egl.CreateImageKHR(
			display,
			egl::NO_CONTEXT,
			egl::LINUX_DMA_BUF_EXT,
			std::ptr::null_mut(),
			attribs.as_ptr(),
		);

		if image == egl::NO_IMAGE_KHR {
			let err = egl.GetError();
			error!(
				egl_error = format_args!("0x{err:04x}"),
				"Failed to create EGLImage from DMA-BUF"
			);
			return Err(ExternalTextureError::EglImageCreationFailed(err));
		}

		debug!(?image, "Successfully created EGLImage from DMA-BUF");

		// -------------------------------------------------------------------------
		// Create GL texture
		// -------------------------------------------------------------------------

		let mut tex = 0u32;
		gl!(gl, GenTextures(1, &mut tex));

		if tex == 0 {
			error!("glGenTextures returned texture = 0");
			return Err(ExternalTextureError::GlTextureFailed);
		}

		gl!(gl, BindTexture(gl::TEXTURE_2D, tex));

		gl!(gl, EGLImageTargetTexture2DOES(gl::TEXTURE_2D, image));

		gl!(
			gl,
			TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32)
		);
		gl!(
			gl,
			TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32)
		);

		gl!(gl, BindTexture(gl::TEXTURE_2D, 0));

		debug!(
			texture = tex,
			width = payload.width,
			height = payload.height,
			"Imported DMA-BUF into OpenGL texture"
		);

		Ok(Self {
			gl: gl.clone(),
			egl: egl.clone(),
			texture: tex,
			image,
			fd,
			width: payload.width,
			height: payload.height,
		})
	}

	pub fn bind(&self, slot: u32) -> TextureBindGuard {
		TextureBindGuard::bind(&self.gl, gl::TEXTURE_2D, self.texture, slot)
	}
}

impl Drop for ExternalTexture {
	fn drop(&mut self) {
		unsafe {
			if self.texture != 0 {
				debug!(texture = self.texture, "Deleting GL texture");
				gl!(&self.gl, DeleteTextures(1, &self.texture));
			}

			if self.image != egl::NO_IMAGE_KHR {
				debug!(?self.image, "Destroying EGLImage");
				let display = self.egl.GetCurrentDisplay();
				self.egl.DestroyImageKHR(display, self.image);
			}

			debug!(fd = self.fd, "Closing DMA-BUF file descriptor");
			libc::close(self.fd);
		}
	}
}
