use std::marker::PhantomData;

use easydrm::gl;

pub struct BindingGuard<'a, T, Set>
where
	Set: Fn(&gl::Gles2, T),
	T: Copy,
{
	gl: &'a gl::Gles2,
	previous: T,
	setter: Set,
	_marker: PhantomData<T>,
}

impl<'a, T, Set> BindingGuard<'a, T, Set>
where
	Set: Fn(&gl::Gles2, T),
	T: Copy,
{
	pub fn new(gl: &'a gl::Gles2, previous: T, setter: Set, new_value: T) -> Self {
		setter(gl, new_value);
		Self {
			gl,
			previous,
			setter,
			_marker: PhantomData,
		}
	}
}

impl<'a, T, Set> Drop for BindingGuard<'a, T, Set>
where
	Set: Fn(&gl::Gles2, T),
	T: Copy,
{
	fn drop(&mut self) {
		(self.setter)(self.gl, self.previous);
	}
}

pub fn read_binding(gl: &gl::Gles2, param: u32) -> u32 {
	let mut value = 0;
	gl!(gl, GetIntegerv(param, &mut value));
	value as u32
}
