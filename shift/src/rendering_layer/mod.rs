#![allow(dead_code)]

pub mod channels;
pub mod dmabuf_import;
mod egl;

use easydrm::{gl::{self, COLOR_BUFFER_BIT, DEPTH_BUFFER_BIT}, EasyDRM, Monitor, MonitorContextCreationRequest};
use skia_safe::{
	self as skia, AlphaType, FilterMode, MipmapMode, Paint, SamplingOptions, gpu,
	gpu::gl::FramebufferInfo,
};
use std::{collections::HashMap, hash::Hash, os::fd::OwnedFd};
use tab_protocol::BufferIndex;
use thiserror::Error;
use tracing::warn;

use crate::{
	comms::{
		render2server::{RenderEvt, RenderEvtTx},
		server2render::{RenderCmd, RenderCmdRx},
	},
	monitor::{Monitor as ServerLayerMonitor, MonitorId},
	sessions::SessionId,
};
use channels::RenderingEnd;
use dmabuf_import::{DmaBufTexture, ImportParams as DmaBufImportParams, SkiaDmaBufTexture};
use shift_profiler as profiler;
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
	pub id: MonitorId,
}

impl MonitorRenderState {
	fn new(req: &MonitorContextCreationRequest<'_>) -> Result<Self, RenderError> {
		let interface = gpu::gl::Interface::new_load_with(|s| (req.get_proc_address)(s))
			.ok_or(RenderError::SkiaGlInterface)?;

		let mut gr =
			gpu::direct_contexts::make_gl(interface, None).ok_or(RenderError::SkiaDirectContext)?;

		let surface = skia_surface_for_current_fbo(&mut gr, req.gl, req.width, req.height)?;

		Ok(Self {
			gr,
			surface,
			width: req.width,
			height: req.height,
			gl: req.gl.clone(),
			id: MonitorId::rand(),
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

	pub fn get_server_layer_monitor(monitor: &Monitor<Self>) -> ServerLayerMonitor {
		crate::monitor::Monitor {
			height: monitor.size().1 as _,
			width: monitor.size().0 as _,
			id: monitor.context().id,
			name: format!("Monitor {}", u32::from(monitor.connector_id())),
			refresh_rate: monitor.active_mode().vrefresh(),
		}
	}

	fn draw_texture(&mut self, texture: &SkiaDmaBufTexture) -> Result<(), RenderError> {
		let Some(image) = skia::Image::from_texture(
			&mut self.gr,
			&texture.backend_texture,
			gpu::SurfaceOrigin::TopLeft,
			skia::ColorType::RGBA8888,
			AlphaType::Opaque,
			None,
		) else {
			return Err(RenderError::SkiaSurface);
		};
		let rect = skia::Rect::from_wh(self.width as f32, self.height as f32);
		let sampling = SamplingOptions::new(FilterMode::Nearest, MipmapMode::Nearest);
		let mut paint = Paint::default();
		paint.set_alpha_f(1.0);
		paint.set_argb(255, 255, 0,0);
		self.canvas().draw_rect(rect, &paint);
		paint.set_argb(255, 255, 255, 255);
		self
			.canvas()
			.draw_image_rect_with_sampling_options(image, None, rect, sampling, &paint);

		Ok(())
	}

}

#[derive(Default, Debug)]
struct MonitorSurfaceState {
	current_buffer: Option<BufferSlot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SlotKey {
	monitor_id: MonitorId,
	session_id: SessionId,
	buffer: BufferSlot,
}

impl SlotKey {
	fn new(monitor_id: MonitorId, session_id: SessionId, buffer: BufferSlot) -> Self {
		Self {
			monitor_id,
			session_id,
			buffer,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum BufferSlot {
	Zero,
	One,
}

impl BufferSlot {
	fn from_index(idx: usize) -> Option<Self> {
		match idx {
			0 => Some(Self::Zero),
			1 => Some(Self::One),
			_ => None,
		}
	}
}

impl From<BufferIndex> for BufferSlot {
	fn from(value: BufferIndex) -> Self {
		match value {
			BufferIndex::Zero => BufferSlot::Zero,
			BufferIndex::One => BufferSlot::One,
		}
	}
}

impl From<BufferSlot> for BufferIndex {
	fn from(value: BufferSlot) -> Self {
		match value {
			BufferSlot::Zero => BufferIndex::Zero,
			BufferSlot::One => BufferIndex::One,
		}
	}
}

// -----------------------------
// Rendering layer
// -----------------------------

pub struct RenderingLayer {
	drm: EasyDRM<MonitorRenderState>,
	command_rx: Option<RenderCmdRx>,
	event_tx: RenderEvtTx,
	known_monitors: HashMap<MonitorId, ServerLayerMonitor>,
	monitor_state: HashMap<(MonitorId, SessionId), MonitorSurfaceState>,
	slots: HashMap<SlotKey, SkiaDmaBufTexture>,
	current_session: Option<SessionId>,
}

impl RenderingLayer {
	pub fn init(channels: RenderingEnd) -> Result<Self, RenderError> {
		let (command_rx, event_tx) = channels.into_parts();
		let drm = EasyDRM::init(|req| {
			// O EasyDRM chama isto com o contexto do monitor já válido/current.
			MonitorRenderState::new(req).expect("MonitorRenderState::new failed")
		})?;

		Ok(Self {
			drm,
			command_rx: Some(command_rx),
			event_tx,
			known_monitors: HashMap::new(),
			monitor_state: HashMap::new(),
			slots: HashMap::new(),
			current_session: None,
		})
	}

	pub async fn run(mut self) -> Result<(), RenderError> {
		let mut command_rx = self
			.command_rx
			.take()
			.expect("render command channel missing");
		let current = self.collect_monitors();
		self
			.emit_event(RenderEvt::Started {
				monitors: current.clone(),
			})
			.await;
		self.known_monitors = current.into_iter().map(|m| (m.id, m)).collect();

		'e: loop {
			profiler::record("render.loop");
			profiler::report_if_due();
			let _loop_span = profiler::span("render.loop");
			// Mantém as surfaces a seguir ao tamanho real do monitor
			let monitor_ids: Vec<MonitorId> = self.drm.monitors().map(|mon| mon.context().id).collect();
			let current_session = self.current_session;
			if let Some(s) = current_session {
				for id in &monitor_ids {
					self.monitor_state.entry((*id, s)).or_default();
				}
			}
			for mon in self.drm.monitors_mut() {
				let _mon_span = profiler::span("render.monitor");
				if mon.can_render() && mon.make_current().is_ok() {
					let _draw_span = profiler::span("render.monitor.draw");
					unsafe{mon.gl().Clear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT); };

					let monitor_id = mon.context().id;
					let mode = mon.active_mode();
					let (w, h) = (mode.size().0 as usize, mode.size().1 as usize);
					let context = mon.context_mut();
					context.ensure_surface_size(w, h)?;

					let texture = current_session
						.and_then(|session_id| {
							self.monitor_state
								.get_mut(&(monitor_id, session_id))
								.and_then(|state| state.current_buffer)
								.map(|buffer| SlotKey::new(monitor_id, session_id, buffer))
						})
						.and_then(|key| {
							self.slots
								.get_mut(&key)
						});
					if let Some(texture) = texture {
						unsafe{context.gl.ClearColor(1.0, 0.0, 0.0, 1.0)};
						let canvas = context.canvas();
						// texture.update();
						if let Err(e) = context.draw_texture(texture) {
							warn!(%monitor_id, "failed to draw client texture: {e:?}");
						}
					}

					context.flush();
				}
			}
			let _swap_span = profiler::span("render.swap_buffers");
			self.drm.swap_buffers()?;
			profiler::record("render.swap_buffers");
			'l: loop {
				tokio::select! {
					cmd = command_rx.recv() => {
						let _cmd_span = profiler::span("render.command.recv");
						if let Some(cmd) = cmd {
							if !self.handle_command(cmd).await? {
								break 'e;
							}
						} else {
							warn!("server→renderer channel closed, shutting down renderer");
							break 'e;
						}
					}
					result = self.drm.poll_events_async() => {
						let _poll_span = profiler::span("render.poll_events");
						result?;
						self.on_after_poll_events().await;
						break 'l;
					}
				}
			}
		};
		warn!("shutting down renderer");
		Ok(())
	}
	async fn on_after_poll_events(&mut self) {
		let _span = profiler::span("render.on_after_poll_events");
		profiler::record("render.page_flip");
		let page_flipped_monitors = self
			.drm
			.monitors()
			.filter(|m| m.can_render())
			.map(|m| m.context().id)
			.collect::<Vec<_>>();
		self
			.emit_event(RenderEvt::PageFlip {
				monitors: page_flipped_monitors,
			})
			.await;
		self.sync_monitors().await;
	}
	pub fn drm(&self) -> &EasyDRM<MonitorRenderState> {
		&self.drm
	}

	fn collect_monitors(&self) -> Vec<ServerLayerMonitor> {
		self
			.drm
			.monitors()
			.map(MonitorRenderState::get_server_layer_monitor)
			.collect()
	}

	async fn sync_monitors(&mut self) {
		let _span = profiler::span("render.sync_monitors");
		let current_list = self.collect_monitors();
		let mut current_map = HashMap::new();
		for monitor in current_list {
			if !self.known_monitors.contains_key(&monitor.id) {
				self
					.emit_event(RenderEvt::MonitorOnline {
						monitor: monitor.clone(),
					})
					.await;
			}
			current_map.insert(monitor.id, monitor);
		}
		let removed_ids = self
			.known_monitors
			.keys()
			.filter(|removed_id| !current_map.contains_key(removed_id))
			.copied()
			.collect::<Vec<_>>();
		for removed_id in removed_ids {
			self
				.emit_event(RenderEvt::MonitorOffline {
					monitor_id: removed_id,
				})
				.await;
			self.monitor_state.retain(|(mon, _), _| *mon != removed_id);
			self.cleanup_monitor_slots(removed_id);
		}
		self.known_monitors = current_map;
	}

	pub fn drm_mut(&mut self) -> &mut EasyDRM<MonitorRenderState> {
		&mut self.drm
	}

	fn texture_for_monitor(&self, monitor_id: MonitorId) -> Option<&SkiaDmaBufTexture> {
		let session_id = self.current_session?;
		let state = self.monitor_state.get(&(monitor_id, session_id))?;
		let buffer = state.current_buffer?;
		let key = SlotKey::new(monitor_id, session_id, buffer);
		self.slots.get(&key)
	}

	fn cleanup_monitor_slots(&mut self, monitor_id: MonitorId) {
		self.slots.retain(|key, _| key.monitor_id != monitor_id);
	}

	fn cleanup_session_slots(&mut self, session_id: SessionId) {
		self.slots.retain(|key, _| key.session_id != session_id);
	}

	fn import_framebuffers(
		&mut self,
		payload: tab_protocol::FramebufferLinkPayload,
		dma_bufs: [OwnedFd; 2],
		session_id: SessionId,
	) {
		let Ok(monitor_id) = payload.monitor_id.parse::<MonitorId>() else {
			warn!(monitor_id = %payload.monitor_id, "invalid monitor id in framebuffer link");
			return;
		};

		let mut imported = Vec::new();
		let mut found_monitor = false;
		for mon in self.drm.monitors_mut() {
			if mon.context().id != monitor_id {
				continue;
			}
			found_monitor = true;
			if let Err(e) = mon.make_current() {
				warn!(%monitor_id, "failed to make monitor current: {e:?}");
				break;
			}
			let gl = mon.context().gl.clone();
			let proc_loader = |symbol: &str| mon.get_proc_address(symbol);
			for (idx, fd) in dma_bufs.into_iter().enumerate() {
				let Some(slot) = BufferSlot::from_index(idx) else {
					continue;
				};
				let params = DmaBufImportParams {
					width: payload.width,
					height: payload.height,
					stride: payload.stride,
					offset: payload.offset,
					fourcc: payload.fourcc,
					fd,
				};
				match DmaBufTexture::import(&gl, &proc_loader, params).and_then(|texture| {
					texture.to_skia(format!(
						"session_{}_monitor_{}_buffer_{}",
						session_id, monitor_id, idx
					))
				}) {
					Ok(texture) => imported.push((slot, texture)),
					Err(e) => {
						warn!(%monitor_id, ?slot, "failed to import dmabuf: {e:?}");
					}
				}
			}
			break;
		}

		if !found_monitor {
			warn!(%monitor_id, "framebuffer link for unknown monitor");
			return;
		}

		for (slot, texture) in imported {
			let key = SlotKey::new(monitor_id, session_id, slot);
			self.slots.insert(key, texture);
		}
	}
}

impl RenderingLayer {
	async fn handle_command(&mut self, cmd: RenderCmd) -> Result<bool, RenderError> {
		let _span = profiler::span("render.handle_command");
		match cmd {
			RenderCmd::Shutdown => {
				warn!("received shutdown request from server");
				return Ok(false);
			}
			RenderCmd::FramebufferLink {
				payload,
				dma_bufs,
				session_id,
			} => {
				let _span = profiler::span("render.handle_framebuffer_link");
				self.import_framebuffers(payload, dma_bufs, session_id);
			}
			RenderCmd::SetActiveSession { session_id } => {
				let _span = profiler::span("render.handle_set_active_session");
				self.current_session = session_id;
			}
			RenderCmd::SessionRemoved { session_id } => {
				let _span = profiler::span("render.handle_session_removed");
				self.cleanup_session_slots(session_id);
				if self.current_session == Some(session_id) {
					self.current_session = None;
				}
			}
			RenderCmd::SwapBuffers { monitor_id, buffer, session_id } => {
				let _span = profiler::span("render.handle_swap_buffers");
				let slot = BufferSlot::from(buffer);
				self
					.monitor_state
					.entry((monitor_id, session_id))
					.or_default()
					.current_buffer = Some(slot);
			}
		}

		Ok(true)
	}

	async fn emit_event(&self, event: RenderEvt) {
		let _span = profiler::span("render.emit_event");
		if let Err(e) = self.event_tx.send(event).await {
			warn!("failed to send renderer event to server: {e}");
		}
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
