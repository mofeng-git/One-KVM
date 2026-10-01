#ifndef FFMPEG_RAM_ENCODE_STATS_H
#define FFMPEG_RAM_ENCODE_STATS_H

#include <stdint.h>

typedef struct RamEncodeStats {
  uint64_t input_frames;
  uint64_t borrowed_frames;
  uint64_t copied_frames;
  uint64_t copied_bytes;
  uint64_t prepare_us;
  uint64_t send_us;
  uint64_t receive_us;
  uint64_t packet_us;
} RamEncodeStats;

#endif
