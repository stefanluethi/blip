# Cross-compiles for Arm Cortex-M with arm-none-eabi-gcc, e.g.
#
#   cmake -B build-m0 -DCMAKE_TOOLCHAIN_FILE=cmake/arm-none-eabi.cmake -DSONDE_CPU=cortex-m0

set(CMAKE_SYSTEM_NAME Generic)
set(CMAKE_SYSTEM_PROCESSOR arm)

set(CMAKE_C_COMPILER arm-none-eabi-gcc)
set(CMAKE_CXX_COMPILER arm-none-eabi-g++)
# there is no startup code to link a test program with
set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)

set(SONDE_CPU cortex-m4 CACHE STRING "The -mcpu of the target")
set(CMAKE_C_FLAGS_INIT "-mcpu=${SONDE_CPU} -mthumb -ffunction-sections -fdata-sections")
set(CMAKE_CXX_FLAGS_INIT "${CMAKE_C_FLAGS_INIT} -fno-exceptions -fno-rtti")
