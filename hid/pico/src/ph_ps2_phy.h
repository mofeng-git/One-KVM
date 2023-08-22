#pragma once

#include "ph_types.h"
#include "hardware/pio.h"
#include "pico/util/queue.h"

typedef void (*rx_callback)(u8 byte);

typedef struct {
  PIO pio;
  uint sm;
  queue_t qbytes;
  queue_t qpacks;
  u8 sent;
  rx_callback rx;
} ph_ps2_phy;

void ph_ps2_phy_init(ph_ps2_phy* this, PIO pio, u8 data_pin, rx_callback rx);
void ph_ps2_phy_task(ph_ps2_phy* this);