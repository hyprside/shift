macro_rules! gl {
	($gl:expr, $fn:ident ( $($arg:expr),* $(,)? )) => {{
		#[allow(unused_unsafe)]
		let (result, err) = unsafe {
			let res = $gl.$fn($($arg),*);
			let err = $gl.GetError();
			(res, err)
		};
		if err != easydrm::gl::NO_ERROR {
			let bt = std::backtrace::Backtrace::capture();
			eprintln!(
				"OpenGL error 0x{err:04X} in {} at {}:{}\n{bt}",
				stringify!($fn),
				file!(),
				line!()
			);
		}
		result
	}};
}
