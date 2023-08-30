#include "ph_outputs.h"
#include "ph_ps2_phy.h"

extern bool ph_g_ps2_mouse_online;

ph_ps2_phy ph_ps2_mouse;
u8 ms_type = 0;
u8 ms_mode = 0;
u8 ms_input_mode = 0;
u8 ms_rate = 100;
u32 ms_magic_seq = 0x00;

u8 buttons = 0;

#define MS_TYPE_STANDARD  0x00
#define MS_TYPE_WHEEL_3   0x03
#define MS_TYPE_WHEEL_5   0x04

#define MS_MODE_IDLE      0
#define MS_MODE_STREAMING 1

#define MS_INPUT_CMD      0
#define MS_INPUT_SET_RATE 1

void ph_ps2_mouse_send(u8 byte) {
  queue_try_add(&ph_ps2_mouse.qbytes, &byte);
}

void ph_ps2_mouse_packet(u8 button, u8 x1, u8 y1) {
  if(ms_mode == MS_MODE_STREAMING) {
    u8 s = (button & 7) + 8;
    u8 x = x1 & 0x7f;
    u8 y = y1 & 0x7f;
    u8 z = 0;
    
    if(x1 >> 7) {
      s += 0x10;
      x += 0x80;
    }
    
    if(y1 >> 7) {
      y = 0x80 - y;
    } else if(y) {
      s += 0x20;
      y = 0x100 - y;
    }
    
    ph_ps2_mouse_send(s);
    ph_ps2_mouse_send(x);
    ph_ps2_mouse_send(y);
    
    if (ms_type == MS_TYPE_WHEEL_3 || ms_type == MS_TYPE_WHEEL_5) {
      /*if(report[3] >> 7) {
        z = 0x8 - z;
      } else if(z) {
        z = 0x10 - z;
      }
    
      if (ms_type == MS_TYPE_WHEEL_5) {
        if (report[0] & 0x8) {
          z += 0x10;
        }
    
        if (report[0] & 0x10) {
          z += 0x20;
        }
      }*/
    
      ph_ps2_mouse_send(z);
    }
  }
}

void ph_ps2_mouse_send_button(u8 button, bool state) {
  // TODO: PS2: Send mouse button
  //   @button - USB button code
  //   @state - true if pressed, false if released
  // The function should take care not to send duplicate events (if needed for PS/2)
  // If the PS2 keyboard is not used (PH_O_IS_MOUSE_PS2 is false), the function should do nothing.
  (void)button; // Remove this
  (void)state; // Remove this
  
  u8 bitval = 1;
  
  button--;
  
  if(state) {
    buttons = buttons | (bitval << button);
  } else {
    buttons = buttons & ~(bitval << button);
  }
  
  ph_ps2_mouse_packet(buttons, 0, 0);
}

void ph_ps2_mouse_send_rel(s8 x1, s8 y1) {
  // TODO: PS2: Send relative move event
  // If the PS2 keyboard is not used (PH_O_IS_MOUSE_PS2 is false), the function should do nothing.
  ph_ps2_mouse_packet(buttons, x1, y1);
}

void ph_ps2_mouse_send_wheel(s8 h, s8 v) {
  (void)h;
  // TODO: PS2: Send wheel. As I understand, PS/2 has no horizontal scrolling, so @h just can be ignored.
  //   @v - vertical scrolling like on USB
  // If the PS2 keyboard is not used (PH_O_IS_MOUSE_PS2 is false), the function should do nothing.
  (void)v; // Remove this
}

void ph_ps2_mouse_receive(u8 byte, u8 prev_byte) {
  
  if(ms_input_mode == MS_INPUT_SET_RATE) {
    ms_rate = byte;
    ms_input_mode = MS_INPUT_CMD;
    ph_ps2_mouse_send(0xfa);
  
    ms_magic_seq = (ms_magic_seq << 8) | byte;
    if(ms_type == MS_TYPE_STANDARD && ms_magic_seq == 0xc86450) {
      ms_type = MS_TYPE_WHEEL_3;
    } else if (ms_type == MS_TYPE_WHEEL_3 && ms_magic_seq == 0xc8c850) {
      ms_type = MS_TYPE_WHEEL_5;
    }
    return;
  }
  
  if(byte != 0xf3) {
    ms_magic_seq = 0x00;
  }
  
  switch(byte) {
    case 0xff: // Reset
      ms_type = MS_TYPE_STANDARD;
      ms_mode = MS_MODE_IDLE;
      ms_rate = 100;
      
      ph_ps2_mouse_send(0xfa);
      ph_ps2_mouse_send(0xaa);
      ph_ps2_mouse_send(ms_type);
    return;
  
    case 0xf6: // Set Defaults
      ms_type = MS_TYPE_STANDARD;
      ms_rate = 100;
    case 0xf5: // Disable Data Reporting
    case 0xea: // Set Stream Mode
      ms_mode = MS_MODE_IDLE;
      ph_ps2_mouse_send(0xfa);
    return;
  
    case 0xf4: // Enable Data Reporting
      ms_mode = MS_MODE_STREAMING;
      ph_ps2_mouse_send(0xfa);
    return;
  
    case 0xf3: // Set Sample Rate
      ms_input_mode = MS_INPUT_SET_RATE;
      ph_ps2_mouse_send(0xfa);
    return;
  
    case 0xf2: // Get Device ID
      ph_ps2_mouse_send(0xfa);
      ph_ps2_mouse_send(ms_type);
    return;
  
    case 0xe9: // Status Request
      ph_ps2_mouse_send(0xfa);
      ph_ps2_mouse_send(0x00); // Bit6: Mode, Bit 5: Enable, Bit 4: Scaling, Bits[2,1,0] = Buttons[L,M,R]
      ph_ps2_mouse_send(0x02); // Resolution
      ph_ps2_mouse_send(ms_rate); // Sample Rate
    return;
  
  // TODO: Implement (more of) these?
  //    case 0xf0: // Set Remote Mode
  //    case 0xee: // Set Wrap Mode
  //    case 0xec: // Reset Wrap Mode
  //    case 0xeb: // Read Data
  //    case 0xe8: // Set Resolution
  //    case 0xe7: // Set Scaling 2:1
  //    case 0xe6: // Set Scaling 1:1
  }
  
  ph_ps2_mouse_send(0xfa);
}

void ph_ps2_mouse_task(void) {
  ph_ps2_phy_task(&ph_ps2_mouse);
}

void ph_ps2_mouse_init(u8 gpio) {
  ph_ps2_phy_init(&ph_ps2_mouse, pio0, gpio, &ph_ps2_mouse_receive);
}
