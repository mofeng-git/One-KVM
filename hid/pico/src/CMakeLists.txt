set(target_name hid)
add_executable(${target_name})

target_sources(${target_name} PRIVATE
	main.c
	ph_outputs.c
	ph_usb.c
	ph_usb_kbd.c
	ph_usb_mouse.c
	ph_cmds.c
	ph_spi.c
	ph_debug.c
)
target_link_options(${target_name} PRIVATE -Xlinker --print-memory-usage)
target_compile_options(${target_name} PRIVATE -Wall -Wextra)
target_include_directories(${target_name} PRIVATE ${CMAKE_CURRENT_LIST_DIR})

target_link_libraries(${target_name} PRIVATE
	pico_stdlib
	pico_unique_id
	hardware_spi
	hardware_watchdog
	tinyusb_device
)
pico_add_extra_outputs(${target_name})
