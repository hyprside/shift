cmake_minimum_required(VERSION 3.16)
project(tab_session_client CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(TAB_CLIENT_ROOT "${CMAKE_CURRENT_LIST_DIR}/../..")
list(APPEND CMAKE_MODULE_PATH "${TAB_CLIENT_ROOT}/cmake")

if (CMAKE_BUILD_TYPE MATCHES "^[Rr]elease" OR CMAKE_BUILD_TYPE MATCHES "RelWithDebInfo" OR CMAKE_BUILD_TYPE MATCHES "MinSizeRel")
    set(TAB_CLIENT_BUILD "${TAB_CLIENT_ROOT}/../target/release")
else ()
    set(TAB_CLIENT_BUILD "${TAB_CLIENT_ROOT}/../target/debug")
endif ()

include(FindTabClient)
tab_client_add_build_target()
find_package(TabClient REQUIRED)

add_executable(tab_session_client main.cpp)
add_dependencies(tab_session_client tab_client)
target_link_libraries(tab_session_client PRIVATE TabClient::TabClient)
target_include_directories(tab_session_client PRIVATE ${TabClient_INCLUDE_DIR})
