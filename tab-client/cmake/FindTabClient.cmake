# Try to locate the Tab client library built by Cargo.
#
# Variables defined on success:
#   TabClient_FOUND
#   TabClient_INCLUDE_DIR
#   TabClient_LIBRARY
#   TabClient_VERSION (optional if pkg-config is available)
#   TabClient::TabClient (imported target)
#
# Hints:
#   TAB_CLIENT_ROOT  - root of the repository (defaults to parent of this file)
#   TAB_CLIENT_BUILD - path to Cargo build output (defaults to ${TAB_CLIENT_ROOT}/target/release)

if (NOT DEFINED TAB_CLIENT_ROOT)
    get_filename_component(TAB_CLIENT_ROOT "${CMAKE_CURRENT_LIST_DIR}/.." ABSOLUTE)
endif ()

# Workspace root (Cargo workspace with Cargo.toml)
if (NOT DEFINED TAB_CLIENT_PROJECT_ROOT)
    get_filename_component(TAB_CLIENT_PROJECT_ROOT "${TAB_CLIENT_ROOT}/.." ABSOLUTE)
endif ()

if (NOT DEFINED TAB_CLIENT_BUILD)
    if (CMAKE_BUILD_TYPE MATCHES "^[Rr]elease" OR CMAKE_BUILD_TYPE MATCHES "RelWithDebInfo" OR CMAKE_BUILD_TYPE MATCHES "MinSizeRel")
        set(TAB_CLIENT_BUILD "${TAB_CLIENT_PROJECT_ROOT}/target/release")
    else ()
        set(TAB_CLIENT_BUILD "${TAB_CLIENT_PROJECT_ROOT}/target/debug")
    endif ()
endif ()

find_path(TabClient_INCLUDE_DIR
    NAMES tab_client.h
    HINTS "${TAB_CLIENT_ROOT}/include"
)

find_library(TabClient_LIBRARY
    NAMES tab_client
    HINTS "${TAB_CLIENT_BUILD}"
)

# Allow configuration to proceed even if the library is not built yet by pointing to the expected path.
if (NOT TabClient_LIBRARY)
    set(TabClient_LIBRARY "${TAB_CLIENT_BUILD}/libtab_client.a")
endif ()

include(FindPackageHandleStandardArgs)
find_package_handle_standard_args(TabClient DEFAULT_MSG TabClient_INCLUDE_DIR TabClient_LIBRARY)

if (TabClient_FOUND)
    if (NOT TARGET TabClient::TabClient)
        add_library(TabClient::TabClient UNKNOWN IMPORTED)
        set_target_properties(TabClient::TabClient PROPERTIES
            IMPORTED_LOCATION "${TabClient_LIBRARY}"
            INTERFACE_INCLUDE_DIRECTORIES "${TabClient_INCLUDE_DIR}"
        )
    endif ()
endif ()

# Helper to create a Cargo build target for tab-client.
# Usage:
#   tab_client_add_build_target(<target_name> [PROFILE release|debug])
# Defaults to PROFILE=release.
function(tab_client_add_build_target)
    # Default target name unless the caller provides one as the first arg.
    set(_args "${ARGV}")
    set(_target_name "tab_client")

    if (NOT _args STREQUAL "")
        list(GET _args 0 _first_arg)
        if (NOT _first_arg STREQUAL "PROFILE")
            set(_target_name "${_first_arg}")
            list(REMOVE_AT _args 0)
        endif ()
    endif ()

    set(options)
    set(oneValueArgs PROFILE)
    cmake_parse_arguments(TCAB "${options}" "${oneValueArgs}" "" ${_args})

    # Infer profile if not provided: treat Release/RelWithDebInfo/MinSizeRel as release; otherwise debug.
    if (NOT TCAB_PROFILE)
        if (CMAKE_BUILD_TYPE MATCHES "^[Rr]elease" OR CMAKE_BUILD_TYPE MATCHES "RelWithDebInfo" OR CMAKE_BUILD_TYPE MATCHES "MinSizeRel")
            set(TCAB_PROFILE "release")
        else ()
            set(TCAB_PROFILE "debug")
        endif ()
    endif ()
    string(TOLOWER "${TCAB_PROFILE}" TCAB_PROFILE)

    if (TCAB_PROFILE STREQUAL "debug")
        set(cargo_profile_args "")
        set(tab_client_build_dir "${TAB_CLIENT_PROJECT_ROOT}/target/debug")
    else ()
        set(cargo_profile_args "--release")
        set(tab_client_build_dir "${TAB_CLIENT_PROJECT_ROOT}/target/release")
    endif ()

    # Expose the build directory for the current profile to callers.
    set(TAB_CLIENT_BUILD "${tab_client_build_dir}" PARENT_SCOPE)

    if (NOT TARGET ${_target_name})
        add_custom_target(${_target_name}
            COMMAND cargo build -p tab-client ${cargo_profile_args}
            WORKING_DIRECTORY "${TAB_CLIENT_PROJECT_ROOT}"
            COMMENT "Building tab-client (${TCAB_PROFILE}) with Cargo"
        )
    endif ()
endfunction()
