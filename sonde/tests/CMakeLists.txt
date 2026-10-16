include(FetchContent)
FetchContent_Declare(Catch2
    URL https://github.com/catchorg/Catch2/archive/refs/tags/v3.9.1.tar.gz
    URL_HASH SHA256=a215c2a723bd7483efd236dc86066842a389cb4e344c61119c978acdf24d39be
)
FetchContent_MakeAvailable(Catch2)

find_package(Threads REQUIRED)

add_executable(sonde_tests
    test_batches.cpp
    test_encoding.cpp
    test_file_sink.cpp
    test_threads.cpp
)
target_link_libraries(sonde_tests PRIVATE sonde::sonde Catch2::Catch2WithMain Threads::Threads)
target_compile_definitions(sonde_tests PRIVATE
    SONDE_TEST_DATA="${CMAKE_CURRENT_SOURCE_DIR}/data"
)
if(MSVC)
    target_compile_options(sonde_tests PRIVATE /W4 /permissive- /utf-8)
    target_compile_definitions(sonde_tests PRIVATE _CRT_SECURE_NO_WARNINGS)
else()
    target_compile_options(sonde_tests PRIVATE -Wall -Wextra -Wpedantic)
endif()

list(APPEND CMAKE_MODULE_PATH ${catch2_SOURCE_DIR}/extras)
include(Catch)
catch_discover_tests(sonde_tests WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR})
