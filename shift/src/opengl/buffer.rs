use easydrm::gl;

use crate::opengl::binding::{BindingGuard, read_binding};
use crate::renderer::RendererError;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferType {
	Float,
	Int,
}

impl BufferType {
	fn type_name(self) -> &'static str {
		match self {
			Self::Float => "f32",
			Self::Int => "i32",
		}
	}

	fn gl_enum(self) -> u32 {
		match self {
			Self::Float => gl::FLOAT,
			Self::Int => gl::INT,
		}
	}
}

pub struct Buffer {
	gl: gl::Gles2,
	id: u32,
	btype: BufferType,
	dimensions: usize,
	size: usize,
}

impl Buffer {
	pub fn new_f32(gl: &gl::Gles2, data: &[f32], dimensions: usize) -> Result<Self, RendererError> {
		Self::from_slice(gl, data, BufferType::Float, dimensions)
	}

	#[allow(dead_code)]
	pub fn new_i32(gl: &gl::Gles2, data: &[i32], dimensions: usize) -> Result<Self, RendererError> {
		Self::from_slice(gl, data, BufferType::Int, dimensions)
	}

	fn from_slice<T>(
		gl: &gl::Gles2,
		data: &[T],
		btype: BufferType,
		dimensions: usize,
	) -> Result<Self, RendererError> {
		Self::validate_dimensions(data.len(), dimensions)?;
		let mut id = 0;
		gl!(gl, GenBuffers(1, &mut id));
		if id == 0 {
			return Err(RendererError::Allocation);
		}
		let buffer = Self {
			gl: gl.clone(),
			id,
			btype,
			dimensions,
			size: data.len(),
		};
		buffer.upload_slice(data);
		Ok(buffer)
	}

	pub fn bind(&self) -> BindingGuard<'_, u32, impl Fn(&gl::Gles2, u32)> {
		BindingGuard::new(
			&self.gl,
			read_binding(&self.gl, gl::ARRAY_BUFFER_BINDING),
			|gl, id| {
				gl!(gl, BindBuffer(gl::ARRAY_BUFFER, id));
			},
			self.id,
		)
	}

	pub fn bind_to_attribute(&self, index: u32) {
		let _guard = self.bind();
		gl!(
			&self.gl,
			VertexAttribPointer(
				index,
				self.dimensions as i32,
				self.btype.gl_enum(),
				gl::FALSE as u8,
				0,
				std::ptr::null()
			)
		);
		gl!(&self.gl, EnableVertexAttribArray(index));
	}

	#[allow(dead_code)]
	pub fn size(&self) -> usize {
		if self.dimensions == 0 {
			0
		} else {
			self.size / self.dimensions
		}
	}

	#[allow(dead_code)]
	pub fn dimensions(&self) -> usize {
		self.dimensions
	}

	#[allow(dead_code)]
	pub fn buffer_type(&self) -> BufferType {
		self.btype
	}

	#[allow(dead_code)]
	pub fn update_f32(&mut self, data: &[f32]) -> Result<(), RendererError> {
		self.update_slice(data, BufferType::Float)
	}

	#[allow(dead_code)]
	pub fn update_i32(&mut self, data: &[i32]) -> Result<(), RendererError> {
		self.update_slice(data, BufferType::Int)
	}

	#[allow(dead_code)]
	fn update_slice<T>(&mut self, data: &[T], requested: BufferType) -> Result<(), RendererError> {
		if self.btype != requested {
			return Err(RendererError::TypeMismatch {
				expected: self.btype.type_name(),
				actual: requested.type_name(),
			});
		}
		Self::validate_dimensions(data.len(), self.dimensions)?;
		self.upload_slice(data);
		self.size = data.len();
		Ok(())
	}

	fn upload_slice<T>(&self, data: &[T]) {
		let _guard = self.bind();
		let byte_len = (std::mem::size_of::<T>() * data.len()) as isize;
		gl!(
			&self.gl,
			BufferData(
				gl::ARRAY_BUFFER,
				byte_len,
				data.as_ptr() as *const std::ffi::c_void,
				gl::STATIC_DRAW
			)
		);
	}

	fn validate_dimensions(len: usize, dimensions: usize) -> Result<(), RendererError> {
		if dimensions == 0 || len % dimensions != 0 {
			Err(RendererError::InvalidDimensions { len, dimensions })
		} else {
			Ok(())
		}
	}
}

impl Drop for Buffer {
	fn drop(&mut self) {
		if self.id != 0 {
			gl!(&self.gl, DeleteBuffers(1, &self.id));
		}
	}
}
