# Static libmp3lame for MSVC. LAME 3.100 ships only an nmake file
# (Makefile.MSVC) that also builds the frontends, so the release build uses
# this minimal equivalent: the encoder library only (no mpglib decoder, no
# NASM MMX code, which is 32-bit only), with the same configMS.h settings.
#
# Usage: cmake -S scripts/ffmpeg/release/lame -B <build> -DLAME_SOURCE_DIR=<lame-3.100>

cmake_minimum_required(VERSION 3.21)
project(mp3lame VERSION 3.100 LANGUAGES C)

if(NOT LAME_SOURCE_DIR OR NOT EXISTS "${LAME_SOURCE_DIR}/libmp3lame/lame.c")
  message(FATAL_ERROR "Set LAME_SOURCE_DIR to an extracted lame-3.100 source tree")
endif()

set(src "${LAME_SOURCE_DIR}/libmp3lame")
configure_file("${LAME_SOURCE_DIR}/configMS.h" "${CMAKE_CURRENT_BINARY_DIR}/config.h" COPYONLY)

# Same list as lamelib_sources in Makefile.MSVC.
add_library(mp3lame STATIC
  ${src}/bitstream.c
  ${src}/encoder.c
  ${src}/fft.c
  ${src}/gain_analysis.c
  ${src}/id3tag.c
  ${src}/lame.c
  ${src}/newmdct.c
  ${src}/psymodel.c
  ${src}/quantize.c
  ${src}/quantize_pvt.c
  ${src}/vector/xmm_quantize_sub.c
  ${src}/set_get.c
  ${src}/vbrquantize.c
  ${src}/reservoir.c
  ${src}/tables.c
  ${src}/takehiro.c
  ${src}/util.c
  ${src}/mpglib_interface.c
  ${src}/VbrTag.c
  ${src}/presets.c
  ${src}/version.c
)
target_include_directories(mp3lame PRIVATE
  "${CMAKE_CURRENT_BINARY_DIR}"
  "${LAME_SOURCE_DIR}/include"
  "${src}"
)
target_compile_definitions(mp3lame PRIVATE HAVE_CONFIG_H _CRT_SECURE_NO_WARNINGS)
if(MSVC)
  # configMS.h narrows doubles to floats on purpose (warning C4305, C4244).
  target_compile_options(mp3lame PRIVATE /W1)
endif()

install(TARGETS mp3lame ARCHIVE DESTINATION lib)
install(FILES "${LAME_SOURCE_DIR}/include/lame.h" DESTINATION include/lame)
