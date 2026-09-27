# Links the C runtime statically into SDL3 (/MT), as the program's own
# crt-static does; SDL3's CMake picks the runtime from this variable, not
# from the /MT the cmake crate passes in the flags.
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded")
