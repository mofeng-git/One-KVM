#include "ph_ps2_phy.h"
#include "ph_ps2_phy.pio.h"

void ph_ps2_phy_init(ph_ps2_phy* this, PIO pio, u8 data_pin, rx_callback rx) {
  queue_init(&this->qbytes, sizeof(u8), 9);
  queue_init(&this->qpacks, sizeof(u8) * 9, 16);
  
  this->pio = pio;
  this->sm = pio_claim_unused_sm(this->pio, true);
  ps2phy_program_init(this->pio, this->sm, pio_add_program(this->pio, &ps2phy_program), data_pin);
  
  this->sent = 0;
  this->rx = rx;
}

u16 ph_ps2_frame(u8 byte) {
  u8 parity = 1;
  for (u8 i = 0; i < 8; i++) {
    parity = parity ^ (byte >> i & 1);
  }
  return ((1 << 10) | (parity << 9) | (byte << 1)) ^ 0x7ff;
}

void ph_ps2_phy_task(ph_ps2_phy* this) {
  u8 i = 0;
  u8 pack[9];
  
  if (!queue_is_empty(&this->qbytes)) {
    u8 byte;
    
    while (i < 9 && queue_try_remove(&this->qbytes, &byte)) {
      i++;
      pack[i] = byte;
    }
    
    pack[0] = i;
    queue_try_add(&this->qpacks, &pack);
  }
  
  if (!queue_is_empty(&this->qpacks) && pio_sm_is_tx_fifo_empty(this->pio, this->sm) && !pio_interrupt_get(this->pio, 0)) {
    if (queue_try_peek(&this->qpacks, &pack)) {
      if (this->sent == pack[0]) {
        this->sent = 0;
        queue_try_remove(&this->qpacks, &pack);
      } else {
        this->sent++;
        pio_sm_put(this->pio, this->sm, ph_ps2_frame(pack[this->sent]));
      }
    }
  }
  
  if (!pio_sm_is_rx_fifo_empty(this->pio, this->sm)) {
    u32 fifo = pio_sm_get(this->pio, this->sm);
    fifo = fifo >> 23;
    
    u8 parity = 1;
    for (i = 0; i < 8; i++) {
      parity = parity ^ (fifo >> i & 1);
    }
    
    if (parity != fifo >> 8) {
      //ph_ps2_kbd_send(0xfe);
      return;
    }
    
    (*this->rx)(fifo);
  }
}
