use std::{env, fs::File, path::PathBuf};

use gl_generator::{Api, Fallbacks, Profile, Registry};

fn main() {
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

	let egl_path = out_dir.join("egl_bindings.rs");
	let mut egl_file = File::create(&egl_path).unwrap();
	Registry::new(
		Api::Egl,
		(1, 5),
		Profile::Core,
		Fallbacks::All,
		&[
			"EGL_KHR_image_base",
			"EGL_EXT_image_dma_buf_import",
			"EGL_EXT_image_dma_buf_import_modifiers",
			"EGL_MESA_image_dma_buf_export",
			"EGL_KHR_surfaceless_context",
			"EGL_EXT_platform_base",
			"EGL_KHR_platform_gbm",
			"EGL_MESA_platform_gbm",
		],
	)
	.write_bindings(gl_generator::StructGenerator, &mut egl_file)
	.unwrap();

	let gl_path = out_dir.join("gl_bindings.rs");
	let mut gl_file = File::create(&gl_path).unwrap();
	Registry::new(
		Api::Gles2,
		(3, 2),
		Profile::Core,
		Fallbacks::All,
		&[
			"GL_OES_EGL_image",
			"GL_OES_EGL_image_external",
			"GL_EXT_memory_object_fd",
		],
	)
	.write_bindings(gl_generator::StructGenerator, &mut gl_file)
	.unwrap();

	println!("cargo:rustc-link-lib=EGL");
	println!("cargo:rustc-link-lib=GLESv2");
	println!("cargo:rustc-link-lib=gbm");
	println!("cargo:rerun-if-changed=build.rs");
}
