set(target_name hid)
add_executable(${target_name})

target_sources(${target_name} PRIVATE
	main.c
	ph_outputs.c
	ph_usb.c
	ph_usb_kbd.c
	ph_usb_mouse.c
	ph_ps2.c
	ph_cmds.c
	ph_com.c
	ph_com_bridge.c
	ph_com_spi.c
	ph_com_uart.c
	ph_debug.c

	${PS2_PATH}/ps2phy.c
	${PS2_PATH}/ps2kb.c
	${PS2_PATH}/ps2ms.c
)
target_link_options(${target_name} PRIVATE -Xlinker --print-memory-usage)
target_compile_options(${target_name} PRIVATE -Wall -Wextra)
target_include_directories(${target_name} PRIVATE ${CMAKE_CURRENT_LIST_DIR} ${PS2_PATH})

pico_generate_pio_header(${target_name} ${PS2_PATH}/ps2phy.pio)

target_link_libraries(${target_name} PRIVATE
	pico_stdlib
	pico_unique_id
	hardware_pio
	hardware_spi
	hardware_watchdog
	tinyusb_device
)
pico_add_extra_outputs(${target_name})
