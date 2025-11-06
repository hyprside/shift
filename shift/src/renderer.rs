use easydrm::gl;
use thiserror::Error;

use crate::opengl::{Buffer, BufferGroup, Shader};

#[derive(Debug, Error)]
pub enum RendererError {
	#[error("failed to compile shader: {0}")]
	Shader(String),
	#[error("failed to link program: {0}")]
	Program(String),
	#[error("failed to allocate GL resource")]
	Allocation,
	#[error("buffer length {len} is not divisible by dimensions {dimensions}")]
	InvalidDimensions { len: usize, dimensions: usize },
	#[allow(dead_code)]
	#[error("invalid buffer index {0}")]
	InvalidBufferIndex(usize),
	#[allow(dead_code)]
	#[error("buffer stores {expected} data but {actual} was provided")]
	TypeMismatch {
		expected: &'static str,
		actual: &'static str,
	},
}

const QUAD_POSITIONS: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];

const QUAD_TEX_COORDS: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];

const VERT_SHADER: &str = r#"
#version 330 core
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_tex_coord;
out vec2 v_tex_coord;

void main() {
	v_tex_coord = a_tex_coord;
	gl_Position = vec4(a_position, 0.0, 1.0);
}
"#;

const FRAG_SHADER: &str = r#"
#version 330 core
in vec2 v_tex_coord;
uniform sampler2D u_primary;
uniform sampler2D u_secondary;
uniform float u_mix;
uniform bool u_use_secondary;
out vec4 frag_color;

void main() {
	vec4 base = texture(u_primary, v_tex_coord);
	if (u_use_secondary) {
		vec4 next = texture(u_secondary, v_tex_coord);
		frag_color = mix(base, next, clamp(u_mix, 0.0, 1.0));
	} else {
		frag_color = base;
	}
}
"#;

pub struct MonitorRenderer {
	gl: gl::Gles2,
	shader: Shader,
	geometry: BufferGroup,
	primary_sampler: i32,
	secondary_sampler: i32,
	mix_uniform: i32,
	use_secondary_uniform: i32,
}

impl MonitorRenderer {
	pub fn new(gl: &gl::Gles2) -> Result<Self, RendererError> {
		let shader = Shader::new(gl, VERT_SHADER, FRAG_SHADER)?;
		let mut geometry = BufferGroup::new(gl)?;
		let position_buffer = Buffer::new_f32(gl, &QUAD_POSITIONS, 2)?;
		let tex_coord_buffer = Buffer::new_f32(gl, &QUAD_TEX_COORDS, 2)?;
		let position_attr = shader.attrib_location("a_position") as u32;
		let tex_coord_attr = shader.attrib_location("a_tex_coord") as u32;
		geometry.add_buffer(position_buffer, position_attr);
		geometry.add_buffer(tex_coord_buffer, tex_coord_attr);
		let primary_sampler = shader.uniform_location("u_primary");
		let secondary_sampler = shader.uniform_location("u_secondary");
		let mix_uniform = shader.uniform_location("u_mix");
		let use_secondary_uniform = shader.uniform_location("u_use_secondary");
		Ok(Self {
			gl: gl.clone(),
			shader,
			geometry,
			primary_sampler,
			secondary_sampler,
			mix_uniform,
			use_secondary_uniform,
		})
	}

	pub fn draw(
		&self,
		primary: &crate::dma_buf_importer::ExternalTexture,
		secondary: Option<&crate::dma_buf_importer::ExternalTexture>,
		mix: f32,
	) {
		let _program = self.shader.bind();
		let _vao = self.geometry.bind();
		let _primary_tex = primary.bind(0);
		gl!(&self.gl, Uniform1i(self.primary_sampler, 0));
		let _secondary_guard = if let Some(tex) = secondary {
			let guard = tex.bind(1);
			gl!(&self.gl, Uniform1i(self.secondary_sampler, 1));
			gl!(&self.gl, Uniform1f(self.mix_uniform, mix));
			gl!(&self.gl, Uniform1i(self.use_secondary_uniform, 1));
			Some(guard)
		} else {
			gl!(&self.gl, Uniform1f(self.mix_uniform, 0.0));
			gl!(&self.gl, Uniform1i(self.use_secondary_uniform, 0));
			None
		};
		gl!(&self.gl, DrawArrays(gl::TRIANGLE_STRIP, 0, 4));
	}
}
