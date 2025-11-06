# Tab Client Library

This crate packages Tab v1 client functionality for both Rust and C/C++ consumers.

## Rust usage

Add to your `Cargo.toml` (in this workspace it is already a member):

```toml
tab-client = { path = "../tab-client" }
```

Then:

```rust
use tab_client::TabClient;
use tab_protocol::TabMessageFrame;

let token = std::env::var("SHIFT_SESSION_TOKEN")?;
let mut client = TabClient::connect_default(token)?;
client.send(&TabMessageFrame::raw("ping", "{}"))?;
let reply = client.receive()?;
println!("Got {:?}", reply);
```

### Headless contexts

`tab-client` always renders off-screen, so it always brings up a surfaceless EGL context using GBM on a render node (`/dev/dri/renderD128`, etc.). Set `TAB_CLIENT_RENDER_NODE` if you need to point at a specific render node (e.g. `TAB_CLIENT_RENDER_NODE=/dev/dri/renderD129 cargo run ...`).

## How to use with C++
This library has first-class support for bindings for C and C++, allowing you to integrate into your already existing compositor effortlessly.

If you prefer not to install anything system-wide, let CMake download and build the crate in your tree:

```cmake
include(FetchContent)
FetchContent_Declare(shift_tab
  GIT_REPOSITORY https://github.com/yourorg/shift.git
  GIT_TAG main            # pin to a tag/commit you trust
)
FetchContent_MakeAvailable(shift_tab)

# Import the CMake library
set(TAB_CLIENT_ROOT "${shift_tab_SOURCE_DIR}/tab-client")
list(APPEND CMAKE_MODULE_PATH "${TAB_CLIENT_ROOT}/cmake")

# Add tab_client target
tab_client_add_build_target()

find_package(TabClient REQUIRED)
add_executable(my_app main.cpp)
add_dependencies(my_app tab_client)
target_link_libraries(my_app PRIVATE TabClient::TabClient)
target_include_directories(my_app PRIVATE ${TabClient_INCLUDE_DIR})
```

`tab_client_add_build_target` helper (from `FindTabClient.cmake`) adds a target for this library, making cmake build the rust library automatically. This keeps the exact version you pinned via `GIT_TAG` and avoids requiring users to pre-install the library system-wide. The library is lightweight by design so don't worry about long compile times.

## Local CMake demo

A tiny CMake project that builds and links against `tab-client` lives at `tab-client/examples/cmake-demo`. To try it:

```bash
cmake -S tab-client/examples/cmake-demo -B /tmp/tab-client-demo-build
cmake --build /tmp/tab-client-demo-build
```

The demo calls `tab_client_connect_default(token)` using `SHIFT_SESSION_TOKEN` from the environment and prints the negotiated session info.

## Examples

- `cargo run -p tab-client --example admin_client` launches an interactive admin console (requires `SHIFT_ADMIN_TOKEN`) that can create new sessions/tokens.
- `cargo run -p tab-client --example dvd_rust` renders the classic bouncing DVD logo using the Rust API. Provide a session token via `SHIFT_SESSION_TOKEN` (or pass it as the first CLI arg) to see live rendering on Shift.
- `tab-client/examples/dvd-client-cpp` is a C++ port of the bouncing DVD demo that links against the C bindings (run `examples/dvd-client-cpp/run.sh` after setting `SHIFT_SESSION_TOKEN`).
- `tab-client/examples/session-client` contains a small C++ “normal” client that authenticates with a session token and stays connected until you hit Enter.
