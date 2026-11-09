/* SPDX-License-Identifier: BSD-3-Clause */

#include "Version.h"
#include "common.h"

#define STUB_PIXEL_PLOT(NAME) \
	INTERNAL int NAME(struct zint_symbol* symbol, const unsigned char* pixelbuf) \
	{ \
		(void)symbol; \
		(void)pixelbuf; \
		return ZINT_ERROR_ENCODING_PROBLEM; \
	}

#define STUB_FUNC_CHAR(NAME) \
	INTERNAL int NAME(struct zint_symbol* symbol, unsigned char source[], int length) \
	{ \
		(void)symbol; \
		(void)source; \
		(void)length; \
		strcpy(symbol->errtxt, "Symbology " #NAME " not implemented in embedded libzint"); \
		return ZINT_ERROR_ENCODING_PROBLEM; \
	}

#define STUB_FUNC_SEGS(NAME) \
	INTERNAL int NAME(struct zint_symbol* symbol, struct zint_seg segs[], const int seg_count) \
	{ \
		(void)symbol; \
		(void)segs; \
		(void)seg_count; \
		strcpy(symbol->errtxt, "Symbology " #NAME " not implemented in embedded libzint"); \
		return ZINT_ERROR_ENCODING_PROBLEM; \
	}

STUB_PIXEL_PLOT(zint_png_pixel_plot)
STUB_PIXEL_PLOT(zint_bmp_pixel_plot)
STUB_PIXEL_PLOT(zint_pcx_pixel_plot)
STUB_PIXEL_PLOT(zint_gif_pixel_plot)
STUB_PIXEL_PLOT(zint_tif_pixel_plot)

INTERNAL int zint_ps_plot(struct zint_symbol* symbol)
{
	(void)symbol;
	return ZINT_ERROR_ENCODING_PROBLEM;
}
INTERNAL int zint_emf_plot(struct zint_symbol* symbol, int rotate_angle)
{
	(void)symbol;
	(void)rotate_angle;
	return ZINT_ERROR_ENCODING_PROBLEM;
}

STUB_FUNC_CHAR(zint_c25standard)
STUB_FUNC_CHAR(zint_c25ind)
STUB_FUNC_CHAR(zint_c25iata)
STUB_FUNC_CHAR(zint_c25logic)
STUB_FUNC_CHAR(zint_ean14)
STUB_FUNC_CHAR(zint_code11)
STUB_FUNC_CHAR(zint_msi_plessey)
STUB_FUNC_CHAR(zint_telepen)
STUB_FUNC_CHAR(zint_telepen_num)
STUB_FUNC_CHAR(zint_plessey)
STUB_FUNC_CHAR(zint_flat)
STUB_FUNC_CHAR(zint_fim)
STUB_FUNC_CHAR(zint_postnet)
STUB_FUNC_CHAR(zint_planet)
STUB_FUNC_CHAR(zint_usps_imail)
STUB_FUNC_CHAR(zint_rm4scc)
STUB_FUNC_CHAR(zint_auspost)
STUB_FUNC_CHAR(zint_code16k)
STUB_FUNC_CHAR(zint_composite)
STUB_FUNC_CHAR(zint_kix)
STUB_FUNC_CHAR(zint_daft)
STUB_FUNC_CHAR(zint_nve18)
STUB_FUNC_CHAR(zint_koreapost)
STUB_FUNC_CHAR(zint_japanpost)
STUB_FUNC_CHAR(zint_code49)
STUB_FUNC_CHAR(zint_channel)
STUB_FUNC_SEGS(zint_codeone)
STUB_FUNC_SEGS(zint_gridmatrix)
STUB_FUNC_SEGS(zint_hanxin)
STUB_FUNC_SEGS(zint_dotcode)
STUB_FUNC_SEGS(zint_codablockf)
STUB_FUNC_CHAR(zint_mailmark_2d)
STUB_FUNC_CHAR(zint_mailmark_4s)
STUB_FUNC_CHAR(zint_upu_s10)
STUB_FUNC_SEGS(zint_ultra)
STUB_FUNC_CHAR(zint_dpd)
STUB_FUNC_CHAR(zint_bc412)

#if !ZXING_ENABLE_1D
STUB_FUNC_CHAR(zint_c25inter)
STUB_FUNC_CHAR(zint_itf14)
STUB_FUNC_CHAR(zint_dpleit)
STUB_FUNC_CHAR(zint_dpident)
STUB_FUNC_CHAR(zint_codabar)
STUB_FUNC_CHAR(zint_code39)
STUB_FUNC_CHAR(zint_pzn)
STUB_FUNC_CHAR(zint_code32)
STUB_FUNC_CHAR(zint_vin)
STUB_FUNC_CHAR(zint_pharma)
STUB_FUNC_CHAR(zint_pharma_two)
STUB_FUNC_CHAR(zint_code93)
STUB_FUNC_CHAR(zint_code128)
STUB_FUNC_CHAR(zint_gs1_128)
STUB_FUNC_CHAR(zint_excode39)
STUB_FUNC_CHAR(zint_dbar_exp)
STUB_FUNC_CHAR(zint_dbar_ltd)
STUB_FUNC_CHAR(zint_dbar_omn)
STUB_FUNC_CHAR(zint_eanx)
STUB_FUNC_CHAR(zint_dxfilmedge)
#endif

#if !ZXING_ENABLE_AZTEC
STUB_FUNC_CHAR(zint_aztec)
STUB_FUNC_CHAR(zint_azrune)
#endif

#if !ZXING_ENABLE_DATAMATRIX
STUB_FUNC_SEGS(zint_datamatrix)
#endif

#if !ZXING_ENABLE_MAXICODE
STUB_FUNC_CHAR(zint_maxicode)
#endif

#if !ZXING_ENABLE_PDF417
STUB_FUNC_SEGS(zint_pdf417)
STUB_FUNC_SEGS(zint_micropdf417)
#endif

#if !ZXING_ENABLE_QRCODE
STUB_FUNC_SEGS(zint_qrcode)
STUB_FUNC_SEGS(zint_microqr)
STUB_FUNC_SEGS(zint_rmqr)
STUB_FUNC_SEGS(zint_upnqr)
#endif

