#ifdef __ANDROID__
#include <stddef.h>
#include <stdint.h>
#include <sys/types.h>
#endif

#include <linux/videodev2.h>

#define MARK_FIX_753(name) const unsigned long int Fix753_##name = name;
#include "fix753.h"
