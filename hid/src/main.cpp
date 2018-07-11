#include <Arduino.h>
#include <Keyboard.h>

#define CMD_SERIAL Serial1
#define SERIAL_SPEED 115200

#define INLINE inline __attribute__((always_inline))


INLINE void cmdResetHid() {
	Keyboard.releaseAll();
}

INLINE void cmdKeyEvent() {
	uint8_t state = Serial.read();
	uint8_t key = Serial.read();
	if (state) {
		Keyboard.press(key);
	} else {
		Keyboard.release(key);
	}
}


void setup() {
	CMD_SERIAL.begin(SERIAL_SPEED);
	Keyboard.begin();
}

void loop() {
	while (true) {  // fast
		switch (Serial.read()) {
			case 0: cmdResetHid(); break;
			case 1: cmdKeyEvent(); break;
			default: break;
		}
	}
}
