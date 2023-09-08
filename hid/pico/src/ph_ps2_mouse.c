#include "ph_outputs.h"
#include "ph_ps2_phy.h"

extern bool ph_g_ps2_mouse_online;

ph_ps2_phy ph_ps2_mouse;
bool ph_ps2_mouse_streaming = false;
u32 ph_ps2_mouse_magic_seq = 0;
u8 ph_ps2_mouse_type = 0;
u8 ph_ps2_mouse_buttons = 0;

void ph_ps2_mouse_send(u8 byte) {
  queue_try_add(&ph_ps2_mouse.qbytes, &byte);
}

void ph_ps2_mouse_pack(s8 x, s8 y, s8 h, s8 v) {
  if (ph_ps2_mouse_streaming) {
    u8 byte1 = 0x8 | (ph_ps2_mouse_buttons & 0x7);
    s8 byte2 = x;
    s8 byte3 = 0x100 - y;
    s8 byte4 = 0; // = 0x100 - z;
    
    if (byte2 < 0) byte1 |= 0x10;
    if (byte3 < 0) byte1 |= 0x20;
    
    ph_ps2_mouse_send(byte1);
    ph_ps2_mouse_send(byte2);
    ph_ps2_mouse_send(byte3);
    
    if (ph_ps2_mouse_type == 3 || ph_ps2_mouse_type == 4) {
      //if (byte4 < -8) byte4 = -8;
      //if (byte4 > 7) byte4 = 7;
      //if (byte4 < 0) byte4 |= 0xf8;
      
      if (v < 0) byte4 = 0x01;
      if (v > 0) byte4 = 0xff;
      if (h < 0) byte4 = 0x02;
      if (h > 0) byte4 = 0xfe;
      
      if (ph_ps2_mouse_type == 4) {
        byte4 &= 0xf;
        byte4 |= (ph_ps2_mouse_buttons << 1) & 0x30;
      }
      
      ph_ps2_mouse_send(byte4);
    }
  }
}

void ph_ps2_mouse_send_button(u8 button, bool state) {
  if (PH_O_IS_MOUSE_PS2) {
    button--;
    
    if (state) {
      ph_ps2_mouse_buttons = ph_ps2_mouse_buttons | (1 << button);
    } else {
      ph_ps2_mouse_buttons = ph_ps2_mouse_buttons & ~(1 << button);
    }
    
    ph_ps2_mouse_pack(0, 0, 0, 0);
  }
}

void ph_ps2_mouse_send_rel(s8 x, s8 y) {
  if (PH_O_IS_MOUSE_PS2) {
    ph_ps2_mouse_pack(x, y, 0, 0);
  }
}

void ph_ps2_mouse_send_wheel(s8 h, s8 v) {
  if (PH_O_IS_MOUSE_PS2) {
    ph_ps2_mouse_pack(0, 0, h, v);
  }
}

void ph_ps2_mouse_receive(u8 byte, u8 prev_byte) {
  switch (prev_byte) {
    case 0xf3: // Set Sample Rate
      ph_ps2_mouse_magic_seq = ((ph_ps2_mouse_magic_seq << 8) | byte) & 0xffffff;
      
      if (ph_ps2_mouse_type == 0 && ph_ps2_mouse_magic_seq == 0xc86450) {
        ph_ps2_mouse_type = 3;
      } else if (ph_ps2_mouse_type == 3 && ph_ps2_mouse_magic_seq == 0xc8c850) {
        ph_ps2_mouse_type = 4;
      }
    break;
    
    default:
      switch (byte) {
        case 0xff: // Reset
          ph_ps2_mouse_streaming = false;
          ph_ps2_mouse_type = 0;
          
          ph_ps2_mouse_send(0xfa);
          ph_ps2_mouse_send(0xaa);
          ph_ps2_mouse_send(ph_ps2_mouse_type);
        return;
        
        case 0xf6: // Set Defaults
          ph_ps2_mouse_streaming = false;
          ph_ps2_mouse_type = 0;
        break;
        
        case 0xf5: // Disable Data Reporting
        case 0xea: // Set Stream Mode
          ph_ps2_mouse_streaming = false;
        break;
        
        case 0xf4: // Enable Data Reporting
          ph_ps2_mouse_streaming = true;
        break;
        
        case 0xf2: // Get Device ID
          ph_ps2_mouse_send(0xfa);
          ph_ps2_mouse_send(ph_ps2_mouse_type);
        return;
        
        case 0xe9: // Status Request
          ph_ps2_mouse_send(0xfa);
          ph_ps2_mouse_send(0x00); // Bit6: Mode, Bit 5: Enable, Bit 4: Scaling, Bits[2,1,0] = Buttons[L,M,R]
          ph_ps2_mouse_send(0x02); // Resolution
          ph_ps2_mouse_send(100);  // Sample Rate
        return;
        
        // TODO: Implement (more of) these?
        // case 0xf0: // Set Remote Mode
        // case 0xee: // Set Wrap Mode
        // case 0xec: // Reset Wrap Mode
        // case 0xeb: // Read Data
        // case 0xe8: // Set Resolution
        // case 0xe7: // Set Scaling 2:1
        // case 0xe6: // Set Scaling 1:1
      }
    break;
  }
  
  ph_ps2_mouse_send(0xfa);
}

void ph_ps2_mouse_task(void) {
  ph_ps2_phy_task(&ph_ps2_mouse);
  ph_g_ps2_mouse_online = ph_ps2_mouse_streaming && !ph_ps2_mouse.busy;
}

void ph_ps2_mouse_init(u8 gpio) {
  ph_ps2_phy_init(&ph_ps2_mouse, pio0, gpio, &ph_ps2_mouse_receive);
}
