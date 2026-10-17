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

# Adds the tinycbor library, in a function so that its options do not reach the project.
function(sonde_add_tinycbor)
    include(FetchContent)
    set(options)
    if(CMAKE_VERSION VERSION_GREATER_EQUAL 3.25)
        list(APPEND options SYSTEM)  # no warnings of sonde for the headers of tinycbor
    endif()
    if(CMAKE_VERSION VERSION_GREATER_EQUAL 3.28)
        list(APPEND options EXCLUDE_FROM_ALL)  # nor the install rules of tinycbor
    endif()
    FetchContent_Declare(tinycbor
        URL https://github.com/intel/tinycbor/archive/refs/tags/v7.0.tar.gz
        URL_HASH SHA256=8b1b76001b9f987677f2ea7aa814fba1f810ff6cbbffa62ea3bac612c55b1a56
        ${options}
    )
    # Only the library: options of tinycbor take these variables (CMP0077), not the cache
    set(CMAKE_POLICY_DEFAULT_CMP0077 NEW)
    set(BUILD_TOOLS OFF)
    set(BUILD_EXAMPLES OFF)
    set(BUILD_TESTING OFF)
    set(WITH_CBOR2JSON OFF)
    # only the conversion to JSON, which is off, needs open_memstream; this saves its
    # check and the warning where it is missing
    set(HAVE_OPEN_MEMSTREAM ON)
    FetchContent_MakeAvailable(tinycbor)
endfunction()

if(SONDE_FETCH_TINYCBOR)
    sonde_add_tinycbor()
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
