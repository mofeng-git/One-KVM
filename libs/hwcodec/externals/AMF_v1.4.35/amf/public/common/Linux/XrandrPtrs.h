///-------------------------------------------------------------------------
///  Copyright © 2020-2022 Advanced Micro Devices, Inc. All rights reserved.
///-------------------------------------------------------------------------

#pragma once

#include <memory>
#include <X11/extensions/Xrandr.h>

typedef std::shared_ptr<XRRScreenResources> XRRScreenResourcesPtr;
typedef std::shared_ptr<XRRCrtcInfo> XRRCrtcInfoPtr;