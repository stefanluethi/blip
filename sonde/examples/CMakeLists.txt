# A sketch of a firmware, which only compiles: the hardware access is declared, not
# defined.
add_library(sonde_example_bare_metal OBJECT bare_metal.cpp)
target_link_libraries(sonde_example_bare_metal PRIVATE sonde::sonde)

if(SONDE_HOSTED)
    add_executable(sonde_example_desktop desktop.cpp)
    target_link_libraries(sonde_example_desktop PRIVATE sonde::sonde)
endif()
