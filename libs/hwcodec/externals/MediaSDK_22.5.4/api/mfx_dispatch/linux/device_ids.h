// Copyright (c) 2022 Intel Corporation
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

#include <stdio.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <algorithm>
#include <string>
#include <vector>

enum eMFXHWType
{
    MFX_HW_UNKNOWN   = 0,
    MFX_HW_SNB       = 0x300000,

    MFX_HW_IVB       = 0x400000,

    MFX_HW_HSW       = 0x500000,
    MFX_HW_HSW_ULT   = 0x500001,

    MFX_HW_VLV       = 0x600000,

    MFX_HW_BDW       = 0x700000,

    MFX_HW_CHT       = 0x800000,

    MFX_HW_SKL       = 0x900000,

    MFX_HW_APL       = 0x1000000,

    MFX_HW_KBL       = 0x1100000,
    MFX_HW_GLK       = MFX_HW_KBL + 1,
    MFX_HW_CFL       = MFX_HW_KBL + 2,

    MFX_HW_CNL       = 0x1200000,

    MFX_HW_ICL       = 0x1400000,
    MFX_HW_ICL_LP    = MFX_HW_ICL + 1,
    MFX_HW_JSL       = 0x1500001,
    MFX_HW_EHL       = 0x1500002,

    MFX_HW_TGL_LP    = 0x1600000,
    MFX_HW_RKL       = MFX_HW_TGL_LP + 2,
    MFX_HW_DG1       = 0x1600003,
};

typedef struct {
    int          device_id;
    eMFXHWType   platform;
} mfx_device_item;

// list of dev ID supported by legacy Media SDK
const mfx_device_item msdkDevIDs[] = {
    /*IVB*/
    { 0x0156, MFX_HW_IVB },   /* GT1 mobile */
    { 0x0166, MFX_HW_IVB },   /* GT2 mobile */
    { 0x0152, MFX_HW_IVB },   /* GT1 desktop */
    { 0x0162, MFX_HW_IVB },   /* GT2 desktop */
    { 0x015a, MFX_HW_IVB },   /* GT1 server */
    { 0x016a, MFX_HW_IVB },   /* GT2 server */
    /*HSW*/
    { 0x0402, MFX_HW_HSW },   /* GT1 desktop */
    { 0x0412, MFX_HW_HSW },   /* GT2 desktop */
    { 0x0422, MFX_HW_HSW },   /* GT2 desktop */
    { 0x041e, MFX_HW_HSW },   /* Core i3-4130 */
    { 0x040a, MFX_HW_HSW },   /* GT1 server */
    { 0x041a, MFX_HW_HSW },   /* GT2 server */
    { 0x042a, MFX_HW_HSW },   /* GT2 server */
    { 0x0406, MFX_HW_HSW },   /* GT1 mobile */
    { 0x0416, MFX_HW_HSW },   /* GT2 mobile */
    { 0x0426, MFX_HW_HSW },   /* GT2 mobile */
    { 0x0C02, MFX_HW_HSW },   /* SDV GT1 desktop */
    { 0x0C12, MFX_HW_HSW },   /* SDV GT2 desktop */
    { 0x0C22, MFX_HW_HSW },   /* SDV GT2 desktop */
    { 0x0C0A, MFX_HW_HSW },   /* SDV GT1 server */
    { 0x0C1A, MFX_HW_HSW },   /* SDV GT2 server */
    { 0x0C2A, MFX_HW_HSW },   /* SDV GT2 server */
    { 0x0C06, MFX_HW_HSW },   /* SDV GT1 mobile */
    { 0x0C16, MFX_HW_HSW },   /* SDV GT2 mobile */
    { 0x0C26, MFX_HW_HSW },   /* SDV GT2 mobile */
    { 0x0A02, MFX_HW_HSW },   /* ULT GT1 desktop */
    { 0x0A12, MFX_HW_HSW },   /* ULT GT2 desktop */
    { 0x0A22, MFX_HW_HSW },   /* ULT GT2 desktop */
    { 0x0A0A, MFX_HW_HSW },   /* ULT GT1 server */
    { 0x0A1A, MFX_HW_HSW },   /* ULT GT2 server */
    { 0x0A2A, MFX_HW_HSW },   /* ULT GT2 server */
    { 0x0A06, MFX_HW_HSW },   /* ULT GT1 mobile */
    { 0x0A16, MFX_HW_HSW },   /* ULT GT2 mobile */
    { 0x0A26, MFX_HW_HSW },   /* ULT GT2 mobile */
    { 0x0D02, MFX_HW_HSW },   /* CRW GT1 desktop */
    { 0x0D12, MFX_HW_HSW },   /* CRW GT2 desktop */
    { 0x0D22, MFX_HW_HSW },   /* CRW GT2 desktop */
    { 0x0D0A, MFX_HW_HSW },   /* CRW GT1 server */
    { 0x0D1A, MFX_HW_HSW },   /* CRW GT2 server */
    { 0x0D2A, MFX_HW_HSW },   /* CRW GT2 server */
    { 0x0D06, MFX_HW_HSW },   /* CRW GT1 mobile */
    { 0x0D16, MFX_HW_HSW },   /* CRW GT2 mobile */
    { 0x0D26, MFX_HW_HSW },   /* CRW GT2 mobile */
    { 0x040B, MFX_HW_HSW }, /*HASWELL_B_GT1 *//* Reserved */
    { 0x041B, MFX_HW_HSW }, /*HASWELL_B_GT2*/
    { 0x042B, MFX_HW_HSW }, /*HASWELL_B_GT3*/
    { 0x040E, MFX_HW_HSW }, /*HASWELL_E_GT1*//* Reserved */
    { 0x041E, MFX_HW_HSW }, /*HASWELL_E_GT2*/
    { 0x042E, MFX_HW_HSW }, /*HASWELL_E_GT3*/

    { 0x0C0B, MFX_HW_HSW }, /*HASWELL_SDV_B_GT1*/ /* Reserved */
    { 0x0C1B, MFX_HW_HSW }, /*HASWELL_SDV_B_GT2*/
    { 0x0C2B, MFX_HW_HSW }, /*HASWELL_SDV_B_GT3*/
    { 0x0C0E, MFX_HW_HSW }, /*HASWELL_SDV_B_GT1*//* Reserved */
    { 0x0C1E, MFX_HW_HSW }, /*HASWELL_SDV_B_GT2*/
    { 0x0C2E, MFX_HW_HSW }, /*HASWELL_SDV_B_GT3*/

    { 0x0A0B, MFX_HW_HSW }, /*HASWELL_ULT_B_GT1*/ /* Reserved */
    { 0x0A1B, MFX_HW_HSW }, /*HASWELL_ULT_B_GT2*/
    { 0x0A2B, MFX_HW_HSW }, /*HASWELL_ULT_B_GT3*/
    { 0x0A0E, MFX_HW_HSW }, /*HASWELL_ULT_E_GT1*/ /* Reserved */
    { 0x0A1E, MFX_HW_HSW }, /*HASWELL_ULT_E_GT2*/
    { 0x0A2E, MFX_HW_HSW }, /*HASWELL_ULT_E_GT3*/

    { 0x0D0B, MFX_HW_HSW }, /*HASWELL_CRW_B_GT1*/ /* Reserved */
    { 0x0D1B, MFX_HW_HSW }, /*HASWELL_CRW_B_GT2*/
    { 0x0D2B, MFX_HW_HSW }, /*HASWELL_CRW_B_GT3*/
    { 0x0D0E, MFX_HW_HSW }, /*HASWELL_CRW_E_GT1*/ /* Reserved */
    { 0x0D1E, MFX_HW_HSW }, /*HASWELL_CRW_E_GT2*/
    { 0x0D2E, MFX_HW_HSW }, /*HASWELL_CRW_E_GT3*/

    /* VLV */
    { 0x0f30, MFX_HW_VLV },   /* VLV mobile */
    { 0x0f31, MFX_HW_VLV },   /* VLV mobile */
    { 0x0f32, MFX_HW_VLV },   /* VLV mobile */
    { 0x0f33, MFX_HW_VLV },   /* VLV mobile */
    { 0x0157, MFX_HW_VLV },
    { 0x0155, MFX_HW_VLV },

    /* BDW */
    /*GT3: */
    { 0x162D, MFX_HW_BDW },
    { 0x162A, MFX_HW_BDW },
    /*GT2: */
    { 0x161D, MFX_HW_BDW },
    { 0x161A, MFX_HW_BDW },
    /* GT1: */
    { 0x160D, MFX_HW_BDW },
    { 0x160A, MFX_HW_BDW },
    /* BDW-ULT */
    /* (16x2 - ULT, 16x6 - ULT, 16xB - Iris, 16xE - ULX) */
    /*GT3: */
    { 0x162E, MFX_HW_BDW },
    { 0x162B, MFX_HW_BDW },
    { 0x1626, MFX_HW_BDW },
    { 0x1622, MFX_HW_BDW },
    { 0x1636, MFX_HW_BDW }, /* ULT */
    { 0x163B, MFX_HW_BDW }, /* Iris */
    { 0x163E, MFX_HW_BDW }, /* ULX */
    { 0x1632, MFX_HW_BDW }, /* ULT */
    { 0x163A, MFX_HW_BDW }, /* Server */
    { 0x163D, MFX_HW_BDW }, /* Workstation */

    /* GT2: */
    { 0x161E, MFX_HW_BDW },
    { 0x161B, MFX_HW_BDW },
    { 0x1616, MFX_HW_BDW },
    { 0x1612, MFX_HW_BDW },
    /* GT1: */
    { 0x160E, MFX_HW_BDW },
    { 0x160B, MFX_HW_BDW },
    { 0x1606, MFX_HW_BDW },
    { 0x1602, MFX_HW_BDW },

    /* CHT */
    { 0x22b0, MFX_HW_CHT },
    { 0x22b1, MFX_HW_CHT },
    { 0x22b2, MFX_HW_CHT },
    { 0x22b3, MFX_HW_CHT },

    /* SKL */
    /* GT1F */
    { 0x1902, MFX_HW_SKL }, // DT, 2x1F, 510
    { 0x1906, MFX_HW_SKL }, // U-ULT, 2x1F, 510
    { 0x190A, MFX_HW_SKL }, // Server, 4x1F
    { 0x190B, MFX_HW_SKL },
    { 0x190E, MFX_HW_SKL }, // Y-ULX 2x1F
    /*GT1.5*/
    { 0x1913, MFX_HW_SKL }, // U-ULT, 2x1.5
    { 0x1915, MFX_HW_SKL }, // Y-ULX, 2x1.5
    { 0x1917, MFX_HW_SKL }, // DT, 2x1.5
    /* GT2 */
    { 0x1912, MFX_HW_SKL }, // DT, 2x2, 530
    { 0x1916, MFX_HW_SKL }, // U-ULD 2x2, 520
    { 0x191A, MFX_HW_SKL }, // 2x2,4x2, Server
    { 0x191B, MFX_HW_SKL }, // DT, 2x2, 530
    { 0x191D, MFX_HW_SKL }, // 4x2, WKS, P530
    { 0x191E, MFX_HW_SKL }, // Y-ULX, 2x2, P510,515
    { 0x1921, MFX_HW_SKL }, // U-ULT, 2x2F, 540
    /* GT3 */
    { 0x1923, MFX_HW_SKL }, // U-ULT, 2x3, 535
    { 0x1926, MFX_HW_SKL }, // U-ULT, 2x3, 540 (15W)
    { 0x1927, MFX_HW_SKL }, // U-ULT, 2x3e, 550 (28W)
    { 0x192A, MFX_HW_SKL }, // Server, 2x3
    { 0x192B, MFX_HW_SKL }, // Halo 3e
    { 0x192D, MFX_HW_SKL },
    /* GT4e*/
    { 0x1932, MFX_HW_SKL }, // DT
    { 0x193A, MFX_HW_SKL }, // SRV
    { 0x193B, MFX_HW_SKL }, // Halo
    { 0x193D, MFX_HW_SKL }, // WKS

    /* APL */
    { 0x0A84, MFX_HW_APL },
    { 0x0A85, MFX_HW_APL },
    { 0x0A86, MFX_HW_APL },
    { 0x0A87, MFX_HW_APL },
    { 0x1A84, MFX_HW_APL },
    { 0x1A85, MFX_HW_APL },
    { 0x5A84, MFX_HW_APL },
    { 0x5A85, MFX_HW_APL },

    /* KBL */
    { 0x5902, MFX_HW_KBL }, // DT GT1
    { 0x5906, MFX_HW_KBL }, // ULT GT1
    { 0x5908, MFX_HW_KBL }, // HALO GT1F
    { 0x590A, MFX_HW_KBL }, // SERV GT1
    { 0x590B, MFX_HW_KBL }, // HALO GT1
    { 0x590E, MFX_HW_KBL }, // ULX GT1
    { 0x5912, MFX_HW_KBL }, // DT GT2
    { 0x5913, MFX_HW_KBL }, // ULT GT1 5
    { 0x5915, MFX_HW_KBL }, // ULX GT1 5
    { 0x5916, MFX_HW_KBL }, // ULT GT2
    { 0x5917, MFX_HW_KBL }, // ULT GT2 R
    { 0x591A, MFX_HW_KBL }, // SERV GT2
    { 0x591B, MFX_HW_KBL }, // HALO GT2
    { 0x591C, MFX_HW_KBL }, // ULX GT2
    { 0x591D, MFX_HW_KBL }, // WRK GT2
    { 0x591E, MFX_HW_KBL }, // ULX GT2
    { 0x5921, MFX_HW_KBL }, // ULT GT2F
    { 0x5923, MFX_HW_KBL }, // ULT GT3
    { 0x5926, MFX_HW_KBL }, // ULT GT3 15W
    { 0x5927, MFX_HW_KBL }, // ULT GT3 28W
    { 0x592A, MFX_HW_KBL }, // SERV GT3
    { 0x592B, MFX_HW_KBL }, // HALO GT3
    { 0x5932, MFX_HW_KBL }, // DT GT4
    { 0x593A, MFX_HW_KBL }, // SERV GT4
    { 0x593B, MFX_HW_KBL }, // HALO GT4
    { 0x593D, MFX_HW_KBL }, // WRK GT4
    { 0x87C0, MFX_HW_KBL }, // ULX GT2

    /* GLK */
    { 0x3184, MFX_HW_GLK },
    { 0x3185, MFX_HW_GLK },

    /* CFL */
    { 0x3E90, MFX_HW_CFL },
    { 0x3E91, MFX_HW_CFL },
    { 0x3E92, MFX_HW_CFL },
    { 0x3E93, MFX_HW_CFL },
    { 0x3E94, MFX_HW_CFL },
    { 0x3E96, MFX_HW_CFL },
    { 0x3E98, MFX_HW_CFL },
    { 0x3E99, MFX_HW_CFL },
    { 0x3E9A, MFX_HW_CFL },
    { 0x3E9C, MFX_HW_CFL },
    { 0x3E9B, MFX_HW_CFL },
    { 0x3EA5, MFX_HW_CFL },
    { 0x3EA6, MFX_HW_CFL },
    { 0x3EA7, MFX_HW_CFL },
    { 0x3EA8, MFX_HW_CFL },
    { 0x3EA9, MFX_HW_CFL },
    { 0x87CA, MFX_HW_CFL },

    /* WHL */
    { 0x3EA0, MFX_HW_CFL },
    { 0x3EA1, MFX_HW_CFL },
    { 0x3EA2, MFX_HW_CFL },
    { 0x3EA3, MFX_HW_CFL },
    { 0x3EA4, MFX_HW_CFL },


    /* CML GT1 */
    { 0x9b21, MFX_HW_CFL },
    { 0x9baa, MFX_HW_CFL },
    { 0x9bab, MFX_HW_CFL },
    { 0x9bac, MFX_HW_CFL },
    { 0x9ba0, MFX_HW_CFL },
    { 0x9ba5, MFX_HW_CFL },
    { 0x9ba8, MFX_HW_CFL },
    { 0x9ba4, MFX_HW_CFL },
    { 0x9ba2, MFX_HW_CFL },

    /* CML GT2 */
    { 0x9b41, MFX_HW_CFL },
    { 0x9bca, MFX_HW_CFL },
    { 0x9bcb, MFX_HW_CFL },
    { 0x9bcc, MFX_HW_CFL },
    { 0x9bc0, MFX_HW_CFL },
    { 0x9bc5, MFX_HW_CFL },
    { 0x9bc8, MFX_HW_CFL },
    { 0x9bc4, MFX_HW_CFL },
    { 0x9bc2, MFX_HW_CFL },
    { 0x9bc6, MFX_HW_CFL },
    { 0x9be6, MFX_HW_CFL },
    { 0x9bf6, MFX_HW_CFL },


    /* CNL */
    { 0x5A51, MFX_HW_CNL },
    { 0x5A52, MFX_HW_CNL },
    { 0x5A5A, MFX_HW_CNL },
    { 0x5A40, MFX_HW_CNL },
    { 0x5A42, MFX_HW_CNL },
    { 0x5A4A, MFX_HW_CNL },
    { 0x5A4C, MFX_HW_CNL },
    { 0x5A50, MFX_HW_CNL },
    { 0x5A54, MFX_HW_CNL },
    { 0x5A59, MFX_HW_CNL },
    { 0x5A5C, MFX_HW_CNL },
    { 0x5A41, MFX_HW_CNL },
    { 0x5A44, MFX_HW_CNL },
    { 0x5A49, MFX_HW_CNL },

    /* ICL LP */
    { 0xFF05, MFX_HW_ICL_LP },
    { 0x8A50, MFX_HW_ICL_LP },
    { 0x8A51, MFX_HW_ICL_LP },
    { 0x8A52, MFX_HW_ICL_LP },
    { 0x8A53, MFX_HW_ICL_LP },
    { 0x8A54, MFX_HW_ICL_LP },
    { 0x8A56, MFX_HW_ICL_LP },
    { 0x8A57, MFX_HW_ICL_LP },
    { 0x8A58, MFX_HW_ICL_LP },
    { 0x8A59, MFX_HW_ICL_LP },
    { 0x8A5A, MFX_HW_ICL_LP },
    { 0x8A5B, MFX_HW_ICL_LP },
    { 0x8A5C, MFX_HW_ICL_LP },
    { 0x8A5D, MFX_HW_ICL_LP },
    { 0x8A70, MFX_HW_ICL_LP },
    { 0x8A71, MFX_HW_ICL_LP },  // GT05, but 1 ok in this context

    /* JSL */
    { 0x4E51, MFX_HW_JSL },
    { 0x4E55, MFX_HW_JSL },
    { 0x4E61, MFX_HW_JSL },
    { 0x4E71, MFX_HW_JSL },

    /* EHL */
    { 0x4500, MFX_HW_EHL },
    { 0x4541, MFX_HW_EHL },
    { 0x4551, MFX_HW_EHL },
    { 0x4555, MFX_HW_EHL },
    { 0x4569, MFX_HW_EHL },
    { 0x4571, MFX_HW_EHL },

    /* TGL */
    { 0x9A40, MFX_HW_TGL_LP },
    { 0x9A49, MFX_HW_TGL_LP },
    { 0x9A59, MFX_HW_TGL_LP },
    { 0x9A60, MFX_HW_TGL_LP },
    { 0x9A68, MFX_HW_TGL_LP },
    { 0x9A70, MFX_HW_TGL_LP },
    { 0x9A78, MFX_HW_TGL_LP },

    /* DG1/SG1 */
    { 0x4905, MFX_HW_DG1 },
    { 0x4906, MFX_HW_DG1 },
    { 0x4907, MFX_HW_DG1 },
    { 0x4908, MFX_HW_DG1 },

    /* RKL */
    { 0x4C80, MFX_HW_RKL }, // RKL-S
    { 0x4C8A, MFX_HW_RKL }, // RKL-S
    { 0x4C81, MFX_HW_RKL }, // RKL-S
    { 0x4C8B, MFX_HW_RKL }, // RKL-S
    { 0x4C90, MFX_HW_RKL }, // RKL-S
    { 0x4C9A, MFX_HW_RKL }, // RKL-S
};

typedef struct {
    int vendor_id;
    int device_id;
    eMFXHWType platform;
} Device;

static inline eMFXHWType get_platform(int device_id) {
    for (unsigned i = 0; i < sizeof(msdkDevIDs) / sizeof(msdkDevIDs[0]); ++i) {
        if (msdkDevIDs[i].device_id == device_id) {
            return msdkDevIDs[i].platform;
        }
    }
    return MFX_HW_UNKNOWN;
}

std::vector <Device> get_devices() {
    const char *dir = "/sys/class/drm";
    const char *device_id_file = "/device/device";
    const char *vendor_id_file = "/device/vendor";
    int i = 0;
    int err = 0;
    std::vector <Device> result;
    for (; i < 64; ++i) {
        Device device;
        std::string node_num = std::to_string(128 + i);
        std::string path = std::string(dir) + "/renderD" + node_num + vendor_id_file;
        FILE *file = fopen(path.c_str(), "r");
        if (!file) continue;
        err = fscanf(file, "%x", &device.vendor_id);
        fclose(file);
        if (err == EOF) continue;
        if (device.vendor_id != 0x8086) {  // Filter out non-Intel devices
            continue;
        }
        path = std::string(dir) + "/renderD" + node_num + device_id_file;
        file = fopen(path.c_str(), "r");
        if (!file) continue;
        err = fscanf(file, "%x", &device.device_id);
        fclose(file);
        if (err == EOF) continue;

        // if user only mapped /dev/dri/renderD129 in container, need to skip /dev/dri/renderD128
        path = "/dev/dri/renderD" + node_num;
        int fd = open(path.c_str(), O_RDWR);
        if (fd < 0) continue; //device not accessible
        close(fd);

        device.platform = get_platform(device.device_id);
        result.emplace_back(device);
    }
    std::sort(result.begin(), result.end(), [](const Device &a, const Device &b) {
        return a.platform < b.platform;
    });
    return result;
}
