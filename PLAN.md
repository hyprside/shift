# Shift / Tab Next Steps

This document serves as the implementation plan for bringing the Tab client/server stack
up to the current design requirements (global monitors, outputs, framebuffer‑linking with
double buffering, DVD logo demos, etc.).  Each numbered heading is a major milestone with
ordered sub tasks.  The plan assumes features already landed in `tab-server`/`shift`
regarding global monitor ownership and DMA‑BUF import.

## 1. Protocol & Documentation
1. Enumerate every new/changed message in `tab_protocol::tab::v1` (e.g. `auth_ok`,
   `monitor_{added,removed,updated}`, `framebuffer_link`, `swap_buffers`, `frame_done`,
   `session_ready`).
2. Define JSON schemas using TypeScript syntax (per earlier request) and list field
   semantics (session states including Pending → Loading → Occupied → Consumed).
3. Document monitor layout broadcasting (monitors belong to server; sessions reference IDs).
4. Describe double buffer metadata:
   - `framebuffer_link`: include both textures (buffer indices 0/1) with DMA‑BUF params.
   - `swap_buffers`: include buffer index + implicit fence FD.
   - `frame_done`: expose buffer index ready for reuse.
5. Update docs to note cursor positions, global outputs, and readiness notifications.

## 2. Tab Server Enhancements
1. Keep `TabServer<Texture>` generic and expose `load_dmabuf` closure already wiring into
   Shift’s GL importer (`shift/src/dma_buf_importer.rs`).
2. Maintain a `SessionRegistry`:
   - States: Pending / Loading / Occupied / Consumed.
   - Tokens valid only in Pending.
   - Clean up sessions on disconnect (Consumed).
3. Broadcast monitor layout updates (stub implementation: pack monitors left→right,
   recomputed whenever monitors added/removed).  Add the FIXME explaining this must be
   replaced by user-config layout once registry exists.
4. Map monitors → outputs; store per-output textures handed in via `framebuffer_link`.
5. On `swap_buffers`, validate buffer availability, store fence FD, notify Shift render loop.
6. Emit `frame_done` when Shift finishes scanning a buffer so the compositor can re-use it.

## 3. Tab Client Core (Rust)
1. Build-time: use `gl_generator` for GLES2 + EGL bindings (`build.rs`) similar to
   `experiments/opengl-sender`.
2. Runtime initialization:
   - `TabClient::connect` automatically opens EGL display, chooses config, creates context,
     creates a single shared pbuffer surface (or headless context) and makes it current.
   - Load required extensions (`EGL_EXT_platform_base`, `EGL_KHR_image_base`,
     `EGL_MESA_image_dma_buf_export`, etc.).
3. Data model:
   - `Monitor` struct (ID, dimensions, position, scale, cursor hints).
   - `Output` struct owns two GL textures, FBOs, and exported DMA‑BUF metadata.
   - `TabClient` tracks `Vec<MonitorState>` with `MonitorState { info, output }`.
4. Upon `AuthOkPayload`, synthesize monitors + outputs, run framebuffer_link handshake for each:
   - Create GL textures/FBOs (two buffers) sized to monitor resolution.
   - Export each via `eglExportDMABUFImageMESA`.
   - Send `framebuffer_link` message including both DMA‑BUFs.
5. Provide ergonomic API:
   - `client.monitors()` returns iterator/slice.
   - `Monitor::info()` returns layout metadata; `Monitor::output()` returns `OutputHandle`.
   - `OutputHandle::bind(buffer_index)` binds the FBO for rendering.
   - `OutputHandle::swap_buffers(buffer_index, fence_fd)` sends commit + marks buffer pending.
   - Internal state tracks which buffers are writable (blocked until `frame_done`).
6. Handle server messages:
   - `MonitorAdded/Removed/Updated`: create/destroy outputs, rebalance indices.
   - `SessionStateChanged`: deliver to user if needed.
   - `FrameDone`: mark buffer free; wake any waiter (channel/condvar).
   - Error paths propagate via callbacks or `TabClientError`.
7. Offer blocking + async-friendly surfaces (e.g., allow user to poll for events).

## 4. C API / Headers / Bindings
1. Mirror the Rust API:
   - `tab_client_t`, `tab_monitor_t`, `tab_output_t`.
   - Functions: `tab_client_connect`, `tab_client_monitors`, `tab_monitor_info`,
     `tab_monitor_output`, `tab_output_bind(tab_output_t*, int buffer_index)`,
     `tab_output_swap_buffers(..., int buffer_index, int fence_fd)`, `tab_client_pump_events`.
2. Update `include/tab_client.h` & `src/c_bindings.rs`.
3. Regenerate CMake demo + ensure pkg-config/cmake exports new symbols.

## 5. Rust Example (`examples/dvd_logo.rs`)
1. Load `dvd.png` (via `image` crate).
2. On start, wait for authentication + monitor creation.
3. For each monitor/output:
   - Animate the DVD logo bouncing; keep per-monitor velocity + position.
   - Render into whichever buffer is free (double buffering).
   - On each frame:
     1. Acquire writable buffer (wait for `frame_done` if both busy).
     2. Bind with `output.bind(buffer_index)` → GL viewport/res clearing.
     3. Draw textured quad with DVD texture.
     4. Issue `swap_buffers(buffer_index, fence_fd)` (use `eglCreateSyncKHR`/`dup fence` if avail,
        else submit `-1`/`None` and rely on implicit sync).
4. Hook CLI so pressing Enter quits gracefully (sends session_ready once start-up completes).

## 6. C++ Example (`examples/dvd_logo_cpp/`)
1. Use the C API to drive an equivalent DVD logo animation.
2. Wrap convenience RAII helpers for textures/FBO binding.
3. Share `dvd.png` asset (maybe embed as raw RGBA or load via stb_image).
4. Ensure build via existing CMake demo, update instructions in `README`.

## 7. Testing & Tooling
1. Provide bash one-liner to launch Shift PoC:
   ```bash
   cargo build -p tab-client --example admin_client \
     && cargo run -p shift
   ```
   (already shared; keep doc in README).
2. Add instructions for running Rust + C++ DVD demos (commands + prerequisite env vars).
3. Validate behavior:
   - Run `cargo test -p tab-protocol`.
   - Manual end-to-end: start Shift, admin client auto-spawns, create session tokens, run Rust DVD client, confirm session states + monitor layout updates.

## 8. Follow-ups / Future
1. Replace stub monitor layout with configuration-driven layout (Hypr registry) once ready.
2. Integrate vsync fences more tightly (per-output timeline semaphores or `zwp_linux_dmabuf_v1` analogs).
3. Expand protocol to support multi-buffer (>2) in future if needed.
