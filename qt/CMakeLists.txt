cmake_minimum_required(VERSION 3.21)
project(rvplayer_qt LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 6.10 REQUIRED COMPONENTS Widgets WebEngineWidgets WebChannel Network Test)
qt_standard_project_setup()

# --- Rust core (crates/rv-ffi), always built optimised: the video decoders are too slow otherwise.
set(RV_ROOT ${CMAKE_CURRENT_SOURCE_DIR}/..)
set(RV_FFI_LIB ${RV_ROOT}/target/release/rv_ffi.lib)
add_custom_target(rv_ffi_build
    COMMAND cargo build -p rv-ffi --release
    WORKING_DIRECTORY ${RV_ROOT}
    BYPRODUCTS ${RV_FFI_LIB}
    USES_TERMINAL)
add_library(rv_ffi STATIC IMPORTED)
set_target_properties(rv_ffi PROPERTIES
    IMPORTED_LOCATION ${RV_FFI_LIB}
    INTERFACE_INCLUDE_DIRECTORIES ${RV_ROOT}/crates/rv-ffi/include
    INTERFACE_LINK_LIBRARIES "kernel32;ntdll;userenv;ws2_32;dbghelp")

# --- Everything but main(), shared by the application and the UI tests.
qt_add_library(rvplayer_lib STATIC
    src/app/Startup.cpp src/app/Startup.h
    src/vcl/Dfm.cpp src/vcl/Dfm.h
    src/vcl/AlignLayout.cpp src/vcl/AlignLayout.h
    src/vcl/Widgets.cpp src/vcl/Widgets.h
    src/vcl/Form.cpp src/vcl/Form.h
    src/i18n/Lang.cpp src/i18n/Lang.h
    src/core/Settings.cpp src/core/Settings.h
    src/core/Trace.h
    src/core/Group.h
    src/core/Format.cpp src/core/Format.h
    src/core/MovingInfo.cpp src/core/MovingInfo.h
    src/core/Playback.cpp src/core/Playback.h
    src/draw/Bitmaps.cpp src/draw/Bitmaps.h
    src/draw/Graphics.cpp src/draw/Graphics.h
    src/forms/MainForm.cpp src/forms/MainForm.h
    src/forms/MainForm_Timeline.cpp
    src/forms/MainForm_FileList.cpp
    src/forms/MainForm_Map.cpp
    src/forms/MainForm_Playback.cpp
    src/forms/MainForm_View.cpp
    src/forms/FormFileProperty.cpp src/forms/FormFileProperty.h
    src/forms/FormDetails.cpp src/forms/FormDetails.h
    src/forms/FormAbout.cpp src/forms/FormAbout.h
    src/forms/FormWhatsNew.cpp src/forms/FormWhatsNew.h
    src/forms/FormGroup.cpp src/forms/FormGroup.h
    src/forms/FormCopy.cpp src/forms/FormCopy.h
    src/forms/FormCropping.cpp src/forms/FormCropping.h
    src/forms/FormCroppingItem.cpp src/forms/FormCroppingItem.h
    src/forms/FormRepair.cpp src/forms/FormRepair.h
    src/core/Export.cpp src/core/Export.h
    src/core/RepairEngine.cpp src/core/RepairEngine.h
    src/forms/MainForm_Commands.cpp
    src/forms/MainForm_Coords.cpp
    src/forms/MainForm_Track.cpp
    src/forms/FormGPSLog.cpp src/forms/FormGPSLog.h
    src/forms/FormSetupSubtitleTemplate.cpp src/forms/FormSetupSubtitleTemplate.h
    src/core/GpsFiles.cpp src/core/GpsFiles.h
    src/core/Exif.cpp src/core/Exif.h
    src/video/VideoView.cpp src/video/VideoView.h
    src/filelist/FileManager.cpp src/filelist/FileManager.h
    src/filelist/FileListView.cpp src/filelist/FileListView.h
    src/map/MapView.cpp src/map/MapView.h
)
target_include_directories(rvplayer_lib PUBLIC src)
add_dependencies(rvplayer_lib rv_ffi_build)
target_link_libraries(rvplayer_lib PUBLIC Qt6::Widgets Qt6::WebEngineWidgets Qt6::WebChannel Qt6::Network rv_ffi)

# --- Original Registrator Viewer assets (forms, bitmaps, translations, map pages).
file(GLOB_RECURSE RV_ORIGINAL_ASSETS CONFIGURE_DEPENDS ${RV_ROOT}/assets/original/*)
qt_add_resources(rvplayer_lib "original"
    PREFIX "/original"
    BASE ${RV_ROOT}/assets/original
    FILES ${RV_ORIGINAL_ASSETS})

qt_add_executable(rvplayer WIN32 src/main.cpp)
target_link_libraries(rvplayer PRIVATE rvplayer_lib)

# --- UI tests: drive the real main window with sample recordings (see tests/tst_mainform.cpp).
qt_add_executable(rvplayer_tests tests/tst_mainform.cpp)
target_link_libraries(rvplayer_tests PRIVATE rvplayer_lib Qt6::Test)
enable_testing()
add_test(NAME rvplayer_tests COMMAND rvplayer_tests)
