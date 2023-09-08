#pragma once

#include "ph_types.h"
#include "hardware/pio.h"
#include "pico/util/queue.h"

typedef void (*rx_callback)(u8 byte, u8 prev_byte);

typedef struct {
  PIO pio;
  uint sm;
  queue_t qbytes;
  queue_t qpacks;
  rx_callback rx;
  u8 last_rx;
  u8 last_tx;
  u8 sent;
  u8 busy;
} ph_ps2_phy;

void ph_ps2_phy_init(ph_ps2_phy* this, PIO pio, u8 data_pin, rx_callback rx);
void ph_ps2_phy_task(ph_ps2_phy* this);