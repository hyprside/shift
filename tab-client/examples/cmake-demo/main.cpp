#include "../../include/tab_client.h"
#include <cstdio>
#include <cstdlib>
#include <iostream>

int main() {
    const char* token = std::getenv("SHIFT_SESSION_TOKEN");
    if (!token) {
        std::cout << "Set SHIFT_SESSION_TOKEN before running the demo\n";
        return 1;
    }

    TabClientHandle* client = tab_client_connect_default(token);
    if (!client) {
        std::cout << "tab_client_connect_default failed (is Shift running?)\n";
        return 1;
    }

    char* server = tab_client_get_server_name(client);
    char* protocol = tab_client_get_protocol_name(client);
    char* session = tab_client_get_session_json(client);

    std::cout << "Connected to Shift\n";
    if (server) {
        std::cout << "\tServer name: " << server << "\n";
        tab_client_string_free(server);
    }
    if (protocol) {
        std::cout << "\tProtocol version: " << protocol << "\n";
        tab_client_string_free(protocol);
    }
    if (session) {
        std::cout << "\tSession: " << session << "\n";
        tab_client_string_free(session);
    }

    tab_client_disconnect(client);
    return 0;
}
