use easydrm::gl;

use crate::opengl::binding::{BindingGuard, read_binding};
use crate::opengl::buffer::Buffer;
use crate::renderer::RendererError;

#[allow(dead_code)]
struct StoredBuffer {
	buffer: Buffer,
	attrib_index: u32,
}

pub struct BufferGroup {
	gl: gl::Gles2,
	vao: u32,
	buffers: Vec<StoredBuffer>,
}

impl BufferGroup {
	pub fn new(gl: &gl::Gles2) -> Result<Self, RendererError> {
		let mut vao = 0;
		gl!(gl, GenVertexArrays(1, &mut vao));
		if vao == 0 {
			return Err(RendererError::Allocation);
		}
		Ok(Self {
			gl: gl.clone(),
			vao,
			buffers: Vec::new(),
		})
	}

	pub fn bind(&self) -> BindingGuard<'_, u32, impl Fn(&gl::Gles2, u32)> {
		BindingGuard::new(
			&self.gl,
			read_binding(&self.gl, gl::VERTEX_ARRAY_BINDING),
			|gl, vao| {
				gl!(gl, BindVertexArray(vao));
			},
			self.vao,
		)
	}

	pub fn add_buffer(&mut self, buffer: Buffer, attrib_index: u32) -> usize {
		let index = self.buffers.len();
		self.bind_buffer(&buffer, attrib_index);
		self.buffers.push(StoredBuffer {
			buffer,
			attrib_index,
		});
		index
	}

	#[allow(dead_code)]
	pub fn replace_buffer(
		&mut self,
		buffer: Buffer,
		attrib_index: u32,
		index: usize,
	) -> Result<(), RendererError> {
		if index >= self.buffers.len() {
			return Err(RendererError::InvalidBufferIndex(index));
		}
		self.bind_buffer(&buffer, attrib_index);
		self.buffers[index] = StoredBuffer {
			buffer,
			attrib_index,
		};
		Ok(())
	}

	#[allow(dead_code)]
	pub fn buffer(&self, index: usize) -> Option<&Buffer> {
		self.buffers.get(index).map(|stored| &stored.buffer)
	}

	#[allow(dead_code)]
	pub fn vertices_count(&self) -> usize {
		self
			.buffers
			.iter()
			.map(|b| b.buffer.size())
			.max()
			.unwrap_or(0)
	}

	fn bind_buffer(&self, buffer: &Buffer, attrib_index: u32) {
		let _vao = self.bind();
		buffer.bind_to_attribute(attrib_index);
	}
}

impl Drop for BufferGroup {
	fn drop(&mut self) {
		if self.vao != 0 {
			gl!(&self.gl, DeleteVertexArrays(1, &self.vao));
		}
	}
}
