#include "ph_outputs.h"
#include "ph_ps2_phy.h"

extern u8 ph_g_ps2_kbd_leds;
extern bool ph_g_ps2_kbd_online;

ph_ps2_phy ph_ps2_kbd;
bool ph_ps2_kbd_scanning;
u32 ph_ps2_kbd_repeat_us;
u16 ph_ps2_kbd_delay_ms;
u8 ph_ps2_kbd_repeat = 0;
bool ph_ps2_kbd_repeatmod = false;
alarm_id_t ph_ps2_kbd_repeater;
s8 ph_ps2_is_ctrl = 0;

u8 const ph_ps2_led2ps2[] = { 0, 4, 1, 5, 2, 6, 3, 7 };
u8 const ph_ps2_mod2ps2[] = { 0x14, 0x12, 0x11, 0x1f, 0x14, 0x59, 0x11, 0x27 };
u8 const ph_ps2_hid2ps2[] = {
  0x00, 0x00, 0xfc, 0x00, 0x1c, 0x32, 0x21, 0x23, 0x24, 0x2b, 0x34, 0x33, 0x43, 0x3b, 0x42, 0x4b,
  0x3a, 0x31, 0x44, 0x4d, 0x15, 0x2d, 0x1b, 0x2c, 0x3c, 0x2a, 0x1d, 0x22, 0x35, 0x1a, 0x16, 0x1e,
  0x26, 0x25, 0x2e, 0x36, 0x3d, 0x3e, 0x46, 0x45, 0x5a, 0x76, 0x66, 0x0d, 0x29, 0x4e, 0x55, 0x54,
  0x5b, 0x5d, 0x5d, 0x4c, 0x52, 0x0e, 0x41, 0x49, 0x4a, 0x58, 0x05, 0x06, 0x04, 0x0c, 0x03, 0x0b,
  0x83, 0x0a, 0x01, 0x09, 0x78, 0x07, 0x7c, 0x7e, 0x7e, 0x70, 0x6c, 0x7d, 0x71, 0x69, 0x7a, 0x74,
  0x6b, 0x72, 0x75, 0x77, 0x4a, 0x7c, 0x7b, 0x79, 0x5a, 0x69, 0x72, 0x7a, 0x6b, 0x73, 0x74, 0x6c,
  0x75, 0x7d, 0x70, 0x71, 0x61, 0x2f, 0x37, 0x0f, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38, 0x40,
  0x48, 0x50, 0x57, 0x5f
};
u8 const ph_ps2_maparray = sizeof(ph_ps2_hid2ps2);
u32 const ph_ps2_repeats[] = {
  33333, 37453, 41667, 45872, 48309, 54054, 58480, 62500,
  66667, 75188, 83333, 91743, 100000, 108696, 116279, 125000,
  133333, 149254, 166667, 181818, 200000, 217391, 232558, 250000,
  270270, 303030, 333333, 370370, 400000, 434783, 476190, 500000
};
u16 const ph_ps2_delays[] = { 250, 500, 750, 1000 };

void ph_ps2_kbd_send(u8 byte) {
  queue_try_add(&ph_ps2_kbd.qbytes, &byte);
}

void ph_ps2_kbd_maybe_send_e0(u8 byte) {
  if (byte == 0x46 ||
     (byte >= 0x49 && byte <= 0x52) ||
      byte == 0x54 || byte == 0x58 ||
      byte == 0x65 || byte == 0x66 ||
      byte >= 0x81) {
    ph_ps2_kbd_send(0xe0);
  }
}

int64_t ph_ps2_repeat_callback() {
  if (ph_ps2_kbd_repeat) {
    if (ph_ps2_kbd_repeatmod) {
      
      if (ph_ps2_kbd_repeat > 3 && ph_ps2_kbd_repeat != 6) ph_ps2_kbd_send(0xe0);
      ph_ps2_kbd_send(ph_ps2_mod2ps2[ph_ps2_kbd_repeat - 1]);
      
    } else {
      
      ph_ps2_kbd_maybe_send_e0(ph_ps2_kbd_repeat);
      ph_ps2_kbd_send(ph_ps2_hid2ps2[ph_ps2_kbd_repeat]);
      
    }
    
    return ph_ps2_kbd_repeat_us;
  }
  
  ph_ps2_kbd_repeater = 0;
  return 0;
}

int64_t ph_ps2_blink_callback() {
  ph_g_ps2_kbd_leds = 0;
  ph_ps2_kbd_send(0xaa);
  return 0;
}

void ph_ps2_kbd_reset() {
  ph_ps2_kbd_scanning = true;
  ph_ps2_kbd_repeat_us = 91743;
  ph_ps2_kbd_delay_ms = 500;
  ph_ps2_kbd_repeat = 0;
  ph_g_ps2_kbd_leds = 7;
  add_alarm_in_ms(500, ph_ps2_blink_callback, NULL, false);
}

void ph_ps2_kbd_send_key(u8 key, bool state) {
  if (PH_O_IS_KBD_PS2 && ph_ps2_kbd_scanning) {
    if (key >= 0xe0 && key <= 0xe7) {
      
      if (key == 0xe0 || key == 0xe4) {
        if (state) {
          ph_ps2_is_ctrl++;
        } else {
          ph_ps2_is_ctrl--;
        }
        
        if (ph_ps2_is_ctrl < 0 || ph_ps2_is_ctrl > 2) {
          ph_ps2_is_ctrl = 0;
        }
      }
      
      key -= 0xe0;
      
      if (key > 2 && key != 5) {
        ph_ps2_kbd_send(0xe0);
      }
      
      if (state) {
        ph_ps2_kbd_repeat = key + 1;
        ph_ps2_kbd_repeatmod = true;
        
        if (ph_ps2_kbd_repeater) {
          cancel_alarm(ph_ps2_kbd_repeater);
        }
        
        ph_ps2_kbd_repeater = add_alarm_in_ms(ph_ps2_kbd_delay_ms, ph_ps2_repeat_callback, NULL, false);
      } else {
        if (ph_ps2_kbd_repeat == key + 1 && ph_ps2_kbd_repeatmod) {
          ph_ps2_kbd_repeat = 0;
        }
        
        ph_ps2_kbd_send(0xf0);
      }
      
      ph_ps2_kbd_send(ph_ps2_mod2ps2[key]);
      
    } else if (key < ph_ps2_maparray) {
      
      if (key == 0x48) {
        ph_ps2_kbd_repeat = 0;
        
        if (state) {
          if (ph_ps2_is_ctrl) {
            ph_ps2_kbd_send(0xe0); ph_ps2_kbd_send(0x7e); ph_ps2_kbd_send(0xe0); ph_ps2_kbd_send(0xf0); ph_ps2_kbd_send(0x7e);
          } else {
            ph_ps2_kbd_send(0xe1); ph_ps2_kbd_send(0x14); ph_ps2_kbd_send(0x77); ph_ps2_kbd_send(0xe1);
            ph_ps2_kbd_send(0xf0); ph_ps2_kbd_send(0x14); ph_ps2_kbd_send(0xf0); ph_ps2_kbd_send(0x77);
          }
        }
      } else {
        ph_ps2_kbd_maybe_send_e0(key);
        
        if (state) {
          ph_ps2_kbd_repeat = key;
          ph_ps2_kbd_repeatmod = false;
          
          if (ph_ps2_kbd_repeater) {
            cancel_alarm(ph_ps2_kbd_repeater);
          }
          
          ph_ps2_kbd_repeater = add_alarm_in_ms(ph_ps2_kbd_delay_ms, ph_ps2_repeat_callback, NULL, false);
        } else {
          if (ph_ps2_kbd_repeat == key && !ph_ps2_kbd_repeatmod) {
            ph_ps2_kbd_repeat = 0;
          }
          
          ph_ps2_kbd_send(0xf0);
        }
        
        ph_ps2_kbd_send(ph_ps2_hid2ps2[key]);
      }
    }
  }
}

void ph_ps2_kbd_receive(u8 byte, u8 prev_byte) {
  switch (prev_byte) {
    case 0xed: // Set LEDs
      if (byte > 7) byte = 0;
      ph_g_ps2_kbd_leds = ph_ps2_led2ps2[byte];
    break;
    
    case 0xf3: // Set typematic rate and delay
      ph_ps2_kbd_repeat_us = ph_ps2_repeats[byte & 0x1f];
      ph_ps2_kbd_delay_ms = ph_ps2_delays[(byte & 0x60) >> 5];
    break;
    
    default:
      switch (byte) {
        case 0xff: // Reset
          ph_ps2_kbd_reset();
        break;
        
        case 0xee: // Echo
          ph_ps2_kbd_send(0xee);
        return;
        
        case 0xf2: // Identify keyboard
          ph_ps2_kbd_send(0xfa);
          ph_ps2_kbd_send(0xab);
          ph_ps2_kbd_send(0x83);
        return;
        
        case 0xf4: // Enable scanning
          ph_ps2_kbd_scanning = true;
        break;
        
        case 0xf5: // Disable scanning, restore default parameters
        case 0xf6: // Set default parameters
          ph_ps2_kbd_scanning = byte == 0xf6;
          ph_ps2_kbd_repeat_us = 91743;
          ph_ps2_kbd_delay_ms = 500;
          ph_ps2_kbd_repeat = 0;
          ph_g_ps2_kbd_leds = 0;
        break;
      }
    break;
  }
  
  ph_ps2_kbd_send(0xfa);
}

void ph_ps2_kbd_task(void) {
  ph_ps2_phy_task(&ph_ps2_kbd);
  ph_g_ps2_kbd_online = ph_ps2_kbd_scanning && !ph_ps2_kbd.busy;
}

void ph_ps2_kbd_init(u8 gpio) {
  ph_ps2_phy_init(&ph_ps2_kbd, pio0, gpio, &ph_ps2_kbd_receive); 
  ph_ps2_kbd_reset();
}
