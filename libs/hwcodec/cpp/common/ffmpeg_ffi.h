#ifndef FFMPEG_H
#define FFMPEG_H

#define AV_LOG_QUIET -8
#define AV_LOG_PANIC 0
#define AV_LOG_FATAL 8
#define AV_LOG_ERROR 16
#define AV_LOG_WARNING 24
#define AV_LOG_INFO 32
#define AV_LOG_VERBOSE 40
#define AV_LOG_DEBUG 48
#define AV_LOG_TRACE 56

enum AVPixelFormat {
  AV_PIX_FMT_YUV420P = 0,
  AV_PIX_FMT_YUYV422 = 1,
  AV_PIX_FMT_RGB24 = 2,
  AV_PIX_FMT_BGR24 = 3,
  AV_PIX_FMT_YUV422P = 4,     // planar YUV 4:2:2
  AV_PIX_FMT_YUVJ420P = 12,   // JPEG full-range YUV420P (same layout as YUV420P)
  AV_PIX_FMT_YUVJ422P = 13,   // JPEG full-range YUV422P (same layout as YUV422P)
  AV_PIX_FMT_NV12 = 23,
  AV_PIX_FMT_NV21 = 24,
  AV_PIX_FMT_NV16 = 101,
  AV_PIX_FMT_NV24 = 188,
};

int av_log_get_level(void);
void av_log_set_level(int level);
void hwcodec_set_av_log_callback();
void hwcodec_set_flag_could_not_find_ref_with_poc();

#endif
