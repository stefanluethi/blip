cmake_minimum_required(VERSION 3.16)

project(sonde VERSION 0.1.0 LANGUAGES C CXX)

if(CMAKE_SOURCE_DIR STREQUAL PROJECT_SOURCE_DIR)
    set(sonde_top_level ON)
else()
    set(sonde_top_level OFF)
endif()
if(CMAKE_CROSSCOMPILING)
    set(sonde_hosted_default OFF)
else()
    set(sonde_hosted_default ON)
endif()

option(SONDE_HOSTED "Build the file sinks and the system clock" ${sonde_hosted_default})
option(SONDE_FETCH_TINYCBOR
    "Fetch tinycbor; OFF uses a tinycbor target the project provides" ON)
if(sonde_top_level AND sonde_hosted_default)
    set(sonde_tests_default ON)
else()
    set(sonde_tests_default OFF)
endif()
option(SONDE_BUILD_TESTS "Fetch Catch2 and build the tests" ${sonde_tests_default})
option(SONDE_BUILD_EXAMPLES "Build the examples" ${sonde_top_level})
option(SONDE_WERROR "Treat the warnings of sonde as errors" OFF)
set(SONDE_LOGF_MAX 128 CACHE STRING "Bytes of a logf message")

# --- tinycbor --------------------------------------------------------------------------

if(POLICY CMP0135)
    cmake_policy(SET CMP0135 NEW)  # extracted files get the time of extraction
endif()

if(SONDE_FETCH_TINYCBOR)
    include(FetchContent)
    FetchContent_Declare(tinycbor
        URL https://github.com/intel/tinycbor/archive/refs/tags/v0.6.1.tar.gz
        URL_HASH SHA256=0f9944496d1143935e9c996bc6233ca0dd5451299def33ef400a409942f8f34b
    )
    # tinycbor has no CMakeLists.txt, so this only fetches its sources
    FetchContent_MakeAvailable(tinycbor)

    set(tinycbor_src ${tinycbor_SOURCE_DIR}/src)
    add_library(tinycbor STATIC
        ${tinycbor_src}/cborencoder.c
        ${tinycbor_src}/cborerrorstrings.c
        ${tinycbor_src}/cborparser.c
    )
    target_include_directories(tinycbor SYSTEM PUBLIC ${tinycbor_src})
    set_target_properties(tinycbor PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON)
    if(MSVC)
        target_compile_definitions(tinycbor PRIVATE _CRT_SECURE_NO_WARNINGS)
    endif()
elseif(NOT TARGET tinycbor)
    message(FATAL_ERROR "SONDE_FETCH_TINYCBOR is OFF, but there is no tinycbor target")
endif()

# --- sonde -----------------------------------------------------------------------------

add_library(sonde
    src/encoder.cpp
    src/tracer.cpp
    $<$<BOOL:${SONDE_HOSTED}>:src/file_sink.cpp>
)
add_library(sonde::sonde ALIAS sonde)
target_include_directories(sonde PUBLIC include)
target_compile_features(sonde PUBLIC cxx_std_17)
target_link_libraries(sonde PUBLIC tinycbor)
if(SONDE_HOSTED)
    target_compile_definitions(sonde PUBLIC SONDE_HOSTED=1)
else()
    target_compile_definitions(sonde PUBLIC SONDE_HOSTED=0)
endif()
target_compile_definitions(sonde PRIVATE SONDE_LOGF_MAX=${SONDE_LOGF_MAX})
if(MSVC)
    target_compile_options(sonde PRIVATE /W4 /permissive-)
    target_compile_definitions(sonde PRIVATE _CRT_SECURE_NO_WARNINGS)
else()
    target_compile_options(sonde PRIVATE -Wall -Wextra -Wpedantic -Wshadow -Wconversion)
endif()
if(SONDE_WERROR)
    target_compile_options(sonde PRIVATE $<IF:$<CXX_COMPILER_ID:MSVC>,/WX,-Werror>)
endif()

if(SONDE_BUILD_TESTS)
    if(NOT SONDE_HOSTED)
        message(FATAL_ERROR "the tests need SONDE_HOSTED")
    endif()
    enable_testing()
    add_subdirectory(tests)
endif()
if(SONDE_BUILD_EXAMPLES)
    add_subdirectory(examples)
endif()
