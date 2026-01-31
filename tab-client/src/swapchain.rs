use std::os::fd::OwnedFd;

use gbm::BufferObject;
use tab_protocol::{BufferIndex, FramebufferLinkPayload};

use crate::error::TabClientError;

/// Metadata describing a DMA-BUF-backed buffer.
#[derive(Debug)]
pub struct TabBuffer {
	pub index: BufferIndex,
	bo: BufferObject<()>,
}

impl TabBuffer {
	pub fn new(index: BufferIndex, bo: BufferObject<()>) -> Self {
		Self { index, bo }
	}

	pub fn width(&self) -> i32 {
		self.bo.width() as i32
	}

	pub fn height(&self) -> i32 {
		self.bo.height() as i32
	}

	pub fn stride(&self) -> i32 {
		self.bo.stride() as i32
	}

	pub fn offset(&self) -> i32 {
		self.bo.offset(0) as i32
	}

	pub fn fourcc(&self) -> i32 {
		self.bo.format() as u32 as i32
	}

	pub fn duplicate_fd(&self) -> Result<OwnedFd, TabClientError> {
		Ok(self.bo.fd()?)
	}
}

/// Double-buffer swapchain model.
#[derive(Debug)]
pub struct TabSwapchain {
	pub monitor_id: String,
	pub buffers: [TabBuffer; 2],
	current: BufferIndex,
	last_acquired: Option<BufferIndex>,
}

impl TabSwapchain {
	pub fn new(monitor_id: impl Into<String>, buffers: [TabBuffer; 2]) -> Self {
		Self {
			monitor_id: monitor_id.into(),
			buffers,
			current: BufferIndex::Zero,
			last_acquired: None,
		}
	}

	pub fn acquire_next(&mut self) -> (&TabBuffer, BufferIndex) {
		let next = match self.current {
			BufferIndex::Zero => BufferIndex::One,
			BufferIndex::One => BufferIndex::Zero,
		};
		self.current = next;
		self.last_acquired = Some(next);
		(&self.buffers[next as usize], next)
	}

	pub fn rollback(&mut self) {
		if let Some(last) = self.last_acquired.take() {
			self.current = match last {
				BufferIndex::Zero => BufferIndex::One,
				BufferIndex::One => BufferIndex::Zero,
			};
		}
	}

	pub fn current(&self) -> (&TabBuffer, BufferIndex) {
		(&self.buffers[self.current as usize], self.current)
	}

	pub fn framebuffer_link_payload(&self) -> FramebufferLinkPayload {
		let buffer = &self.buffers[0];
		FramebufferLinkPayload {
			monitor_id: self.monitor_id.clone(),
			width: buffer.width(),
			height: buffer.height(),
			stride: buffer.stride(),
			offset: buffer.offset(),
			fourcc: buffer.fourcc(),
		}
	}

	pub fn export_fds(&self) -> Result<[OwnedFd; 2], TabClientError> {
		let fd0 = self.buffers[0].duplicate_fd()?;
		let fd1 = self.buffers[1].duplicate_fd()?;
		Ok([fd0, fd1])
	}
}
