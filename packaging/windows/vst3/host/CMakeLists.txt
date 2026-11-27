# kabl-vst3-host: loads kabl.vst3 with the VST3 SDK's hosting classes and checks its
# parameters, units and MIDI controller mapping (host.cpp). Built by the parent project as
# an external project, like Steinberg's validator, so the SDK's targets do not meet
# clap-wrapper's.
#
#   VST3_SDK_ROOT  the VST3 SDK checkout the parent project fetched

cmake_minimum_required(VERSION 3.25)

set(VST3_SDK_ROOT "" CACHE PATH "VST3 SDK root")
if(NOT EXISTS "${VST3_SDK_ROOT}/pluginterfaces")
    message(FATAL_ERROR "VST3_SDK_ROOT must name a VST3 SDK checkout: '${VST3_SDK_ROOT}'")
endif()

list(APPEND CMAKE_MODULE_PATH "${VST3_SDK_ROOT}/cmake/modules")
include(SMTG_VST3_SDK)

project(kabl-vst3-host LANGUAGES C CXX)

# What the SDK's own top-level project sets up before it creates the hosting libraries.
smtg_setup_platform_toolset()
set(SDK_ROOT "${VST3_SDK_ROOT}")
set(public_sdk_SOURCE_DIR "${SDK_ROOT}/public.sdk")
set(pluginterfaces_SOURCE_DIR "${SDK_ROOT}/pluginterfaces")
include_directories(${SDK_ROOT})
smtg_create_lib_base_target()
smtg_create_pluginterfaces_target()
smtg_create_public_sdk_common_target()
smtg_create_public_sdk_hosting_target()

add_executable(kabl-vst3-host
    host.cpp
    ${SDK_ROOT}/public.sdk/source/common/memorystream.cpp
    ${SDK_ROOT}/public.sdk/source/vst/hosting/plugprovider.cpp)
# The platform's module loader, as the validator adds it; Linux only for a local compile check.
if(SMTG_WIN)
    target_sources(kabl-vst3-host PRIVATE ${SDK_ROOT}/public.sdk/source/vst/hosting/module_win32.cpp)
    # The editor checks open windows and probe OpenGL (user32 and gdi32 are linked by default).
    target_link_libraries(kabl-vst3-host PRIVATE opengl32)
elseif(SMTG_LINUX)
    target_sources(kabl-vst3-host PRIVATE ${SDK_ROOT}/public.sdk/source/vst/hosting/module_linux.cpp)
    target_link_libraries(kabl-vst3-host PRIVATE dl)
endif()
target_link_libraries(kabl-vst3-host PRIVATE sdk_hosting)
target_compile_features(kabl-vst3-host PRIVATE cxx_std_17)
