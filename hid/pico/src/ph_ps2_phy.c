// Source: https://github.com/No0ne/ps2x2pico/blob/main/ps2phy.c
// replace ps2phy with ph_ps2_phy

#include "ph_ps2_phy.h"
#include "ph_ps2_phy.pio.h"

s8 prog = -1;

u32 ph_ps2_phy_frame(u8 byte) {
  bool parity = 1;
  for (u8 i = 0; i < 8; i++) {
    parity = parity ^ (byte >> i & 1);
  }
  return ((1 << 10) | (parity << 9) | (byte << 1)) ^ 0x7ff;
}

void ph_ps2_phy_init(ph_ps2_phy* this, PIO pio, u8 data_pin, rx_callback rx) {
  if (prog == -1) {
    prog = pio_add_program(pio, &ph_ps2_phy_program);
  }
  
  queue_init(&this->qbytes, sizeof(u8), 9);
  queue_init(&this->qpacks, sizeof(u8) * 9, 16);
  
  this->sm = pio_claim_unused_sm(pio, true);
  ph_ps2_phy_program_init(pio, this->sm, prog, data_pin);
  
  this->pio = pio;
  this->sent = 0;
  this->rx = rx;
  this->last_rx = 0;
  this->last_tx = 0;
  this->busy = 0;
}

void ph_ps2_phy_task(ph_ps2_phy* this) {
  u8 i = 0;
  u8 byte;
  u8 pack[9];
  
  if (!queue_is_empty(&this->qbytes)) {
    while (i < 9 && queue_try_remove(&this->qbytes, &byte)) {
      i++;
      pack[i] = byte;
    }
    
    pack[0] = i;
    queue_try_add(&this->qpacks, &pack);
  }
  
  if (pio_interrupt_get(this->pio, this->sm)) {
    this->busy = 1;
  } else {
    this->busy &= 2;
  }
  
  if (pio_interrupt_get(this->pio, this->sm + 4)) {
    this->sent--;
    pio_interrupt_clear(this->pio, this->sm + 4);
  }
  
  if (!queue_is_empty(&this->qpacks) && pio_sm_is_tx_fifo_empty(this->pio, this->sm) && !this->busy) {
    if (queue_try_peek(&this->qpacks, &pack)) {
      if (this->sent == pack[0]) {
        this->sent = 0;
        queue_try_remove(&this->qpacks, &pack);
      } else {
        this->sent++;
        this->last_tx = pack[this->sent];
        this->busy |= 2;
        pio_sm_put(this->pio, this->sm, ph_ps2_phy_frame(this->last_tx));
      }
    }
  }
  
  if (!pio_sm_is_rx_fifo_empty(this->pio, this->sm)) {
    u32 fifo = pio_sm_get(this->pio, this->sm) >> 23;
    
    bool parity = 1;
    for (i = 0; i < 8; i++) {
      parity = parity ^ (fifo >> i & 1);
    }
    
    if (parity != fifo >> 8) {
      pio_sm_put(this->pio, this->sm, ph_ps2_phy_frame(0xfe));
      return;
    }
    
    if (fifo == 0xfe) {
      pio_sm_put(this->pio, this->sm, ph_ps2_phy_frame(this->last_tx));
      return;
    }
    
    while (queue_try_remove(&this->qbytes, &byte));
    while (queue_try_remove(&this->qpacks, &pack));
    
    (*this->rx)(fifo, this->last_rx);
    this->last_rx = fifo;
  }
}
