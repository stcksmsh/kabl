# Builds kabl.vst3: the kabl CLAP wrapped by free-audio/clap-wrapper as a clap-first,
# single-file VST3. The CLAP code is linked in as the kabl-clap static library, so the
# .vst3 needs no separate kabl.clap at run time.
#
# Inputs, produced by cargo before configuring (see the windows-vst3 job in ci.yml):
#   KABL_CLAP_STATICLIB    kabl_clap.lib from `cargo rustc -p kabl-clap --crate-type staticlib`
#   KABL_CLAP_NATIVE_LIBS  the file rustc wrote with `--print native-static-libs=<file>`
#
# Targets: kabl_vst3 (writes <build>/assets/VST3/kabl.vst3) and vst3-validator
# (Steinberg's validator from the same VST3 SDK, at <build>/vst3sdk-validator/bin/Release).

cmake_minimum_required(VERSION 3.25)

# Rust's MSVC targets link the DLL C runtime; the C++ side has to match it.
# clap-wrapper otherwise picks the static runtime.
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")
set(CMAKE_CXX_STANDARD 17)

project(kabl-vst3 LANGUAGES C CXX)

if(NOT MSVC)
    message(FATAL_ERROR "packaging/windows/vst3 is built with MSVC only")
endif()
# What clap-wrapper sets for itself when it is the top-level project.
add_compile_options(/Zc:__cplusplus)

set(KABL_CLAP_STATICLIB "" CACHE FILEPATH "kabl-clap built as a static library")
set(KABL_CLAP_NATIVE_LIBS "" CACHE FILEPATH "rustc's native-static-libs list for that library")
if(NOT EXISTS "${KABL_CLAP_STATICLIB}")
    message(FATAL_ERROR "KABL_CLAP_STATICLIB does not exist: '${KABL_CLAP_STATICLIB}'")
endif()
if(NOT EXISTS "${KABL_CLAP_NATIVE_LIBS}")
    message(FATAL_ERROR "KABL_CLAP_NATIVE_LIBS does not exist: '${KABL_CLAP_NATIVE_LIBS}'")
endif()

# The VST3 class id, frozen (OD-W5). It is the 16 bytes bda9c239b42389589c969403a7b9d23e,
# which clap-wrapper also derives from the CLAP id dev.stcksmsh.kabl; written here in the
# COM notation that FUID::fromString and the Windows validator use.
set(KABL_VST3_CID "39C2A9BD23B458899C969403A7B9D23E")

# Every fetched dependency is pinned to a release tag. A failed download stops the
# configure step with an error.
include(FetchContent)
FetchContent_Declare(clap
    GIT_REPOSITORY https://github.com/free-audio/clap.git
    GIT_TAG 1.2.10
    GIT_SHALLOW TRUE
    # Download only: clap-wrapper adds it itself through CLAP_SDK_ROOT.
    SOURCE_SUBDIR do-not-add)
FetchContent_Declare(vst3sdk
    GIT_REPOSITORY https://github.com/steinbergmedia/vst3sdk.git
    GIT_TAG v3.8.0_build_66
    GIT_SHALLOW TRUE
    GIT_SUBMODULES base cmake pluginterfaces public.sdk
    SOURCE_SUBDIR do-not-add)
FetchContent_Declare(clap-wrapper
    GIT_REPOSITORY https://github.com/free-audio/clap-wrapper.git
    GIT_TAG v0.18.0
    GIT_SHALLOW TRUE)
FetchContent_MakeAvailable(clap vst3sdk)
set(CLAP_SDK_ROOT "${clap_SOURCE_DIR}")
set(VST3_SDK_ROOT "${vst3sdk_SOURCE_DIR}")
FetchContent_MakeAvailable(clap-wrapper)

file(READ "${KABL_CLAP_NATIVE_LIBS}" native_libs)
string(REGEX REPLACE ".*native-static-libs:" "" native_libs "${native_libs}")
string(STRIP "${native_libs}" native_libs)
separate_arguments(native_libs WINDOWS_COMMAND "${native_libs}")
# rustc lists the C runtime as /defaultlib:msvcrt, which CMAKE_MSVC_RUNTIME_LIBRARY covers.
list(FILTER native_libs EXCLUDE REGEX "^/defaultlib:")

add_library(kabl-clap-impl STATIC IMPORTED)
set_target_properties(kabl-clap-impl PROPERTIES
    IMPORTED_LOCATION "${KABL_CLAP_STATICLIB}"
    INTERFACE_LINK_LIBRARIES "${native_libs}")

make_clapfirst_plugins(
    TARGET_NAME kabl
    IMPL_TARGET kabl-clap-impl
    OUTPUT_NAME kabl
    ENTRY_SOURCE kabl_entry.cpp
    BUNDLE_IDENTIFIER dev.stcksmsh.kabl
    BUNDLE_VERSION 1.0.0
    COPY_AFTER_BUILD FALSE
    WINDOWS_FOLDER_VST3 FALSE
    PLUGIN_FORMATS CLAP VST3
    ASSET_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/assets)

target_compile_definitions(kabl_vst3-clap-wrapper-vst3-lib PRIVATE
    CLAP_VST3_TUID_STRING="${KABL_VST3_CID}")

include(ExternalProject)
ExternalProject_Add(vst3-validator
    SOURCE_DIR ${vst3sdk_SOURCE_DIR}
    BINARY_DIR ${CMAKE_BINARY_DIR}/vst3sdk-validator
    CMAKE_ARGS
        -DSMTG_ENABLE_VST3_PLUGIN_EXAMPLES=OFF
        -DSMTG_ENABLE_VSTGUI_SUPPORT=OFF
    BUILD_COMMAND ${CMAKE_COMMAND} --build <BINARY_DIR> --config Release --target validator
    INSTALL_COMMAND ""
    EXCLUDE_FROM_ALL TRUE)
