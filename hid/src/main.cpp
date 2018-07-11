#include <Arduino.h>
#include <HID-Project.h>

#include "inline.h"
#include "keymap.h"

#define CMD_SERIAL Serial1
#define CMD_SERIAL_SPEED 115200

#define INLINE inline __attribute__((always_inline))


INLINE void cmdResetHid() {
	CMD_SERIAL.read(); // unused now
	CMD_SERIAL.read(); // unused now
	CMD_SERIAL.read(); // unused now
	Keyboard.releaseAll();
}

INLINE void cmdKeyEvent() {
	uint8_t state = CMD_SERIAL.read();
	uint8_t code = keymap(CMD_SERIAL.read());
	CMD_SERIAL.read(); // unused now
	if (code) {
		if (state) {
			Keyboard.press(code);
		} else {
			Keyboard.release(code);
		}
	}
}


void setup() {
	CMD_SERIAL.begin(CMD_SERIAL_SPEED);
	Keyboard.begin();
}

void loop() {
	if (CMD_SERIAL.available() >= 4) {
		switch ((uint8_t)CMD_SERIAL.read()) {
			case 0: cmdResetHid(); break;
			case 1: cmdKeyEvent(); break;
			default: break;
		}
	}
}
