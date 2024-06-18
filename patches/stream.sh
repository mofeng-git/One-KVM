#!/bin/bash

/usr/bin/ustreamer-dump --sink kvmd::ustreamer::jpeg --output - | /usr/bin/ffmpeg  -re -use_wallclock_as_timestamps 1 -i pipe: -rtbufsize 10M -c:v libx264 -pix_fmt yuv420p -preset:v ultrafast -tune:v zerolatency -profile:v baseline -bf 0 -b:v 3M -maxrate:v 5M  -r 10 -g 10 -an  -f rtp rtp://127.0.0.1:5004
