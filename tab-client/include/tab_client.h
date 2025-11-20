#pragma once

#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct TabClientHandle TabClientHandle;
// Connect to the Tab socket at `socket_path` (UTF-8) and authenticate with `token`.
// Pass NULL for `socket_path` to use the default /tmp/shift.sock. `token` must be non-null.
TabClientHandle* tab_client_connect(const char* socket_path, const char* token);

// Convenience helper for default socket path.
static inline TabClientHandle* tab_client_connect_default(const char* token) {
	return tab_client_connect(NULL, token);
}

// Disconnect and free resources associated with the handle.
void tab_client_disconnect(TabClientHandle* handle);

// Retrieve and clear the last error message. Caller must free via tab_client_string_free.
char* tab_client_take_error(TabClientHandle* handle);

char* tab_client_get_server_name(TabClientHandle* handle);
char* tab_client_get_protocol_name(TabClientHandle* handle);
char* tab_client_get_session_json(TabClientHandle* handle);
bool tab_client_send_ready(TabClientHandle* handle);

// Free an error string returned by tab_client_take_error.
void tab_client_string_free(char* s);

#ifdef __cplusplus
} // extern "C"
#endif
