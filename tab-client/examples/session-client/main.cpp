#include "../../include/tab_client.h"

#include <cstdlib>
#include <iostream>

int main(int argc, char** argv) {
    const char* token = nullptr;
    if (argc > 1) {
        token = argv[1];
    } else {
        token = std::getenv("SHIFT_SESSION_TOKEN");
    }

    if (!token) {
        std::cerr << "Provide a session token via SHIFT_SESSION_TOKEN or argv[1]\n";
        return 1;
    }

    TabClientHandle* client = tab_client_connect_default(token);
    if (!client) {
        std::cerr << "tab_client_connect_default failed\n";
        return 1;
    }

    char* server = tab_client_get_server_name(client);
    char* protocol = tab_client_get_protocol_name(client);
    char* session = tab_client_get_session_json(client);

    std::cout << "Connected to Shift normal session\n";
    if (server) {
        std::cout << "  Server: " << server << "\n";
        tab_client_string_free(server);
    }
    if (protocol) {
        std::cout << "  Protocol: " << protocol << "\n";
        tab_client_string_free(protocol);
    }
    if (session) {
        std::cout << "  Session info: " << session << "\n";
        tab_client_string_free(session);
    }

    std::cout << "Press Enter once your compositor is ready..." << std::endl;
    std::cin.get();
    if (!tab_client_send_ready(client)) {
        std::cerr << "tab_client_send_ready failed\n";
    } else {
        std::cout << "Ready signal sent to Shift. Press Enter to disconnect..." << std::endl;
    }
    std::cin.get();

    tab_client_disconnect(client);
    return 0;
}
