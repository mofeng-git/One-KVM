#ifndef DDA_H
#define DDA_H

#include "DDAImpl.h"
#include "Defs.h"

class DemoApplication {
  /// Demo Application Core class
#define returnIfError(x)                                                       \
  if (FAILED(x)) {                                                             \
    printf(__FUNCTION__ ": Line %d, File %s Returning error 0x%08x\n",         \
           __LINE__, __FILE__, x);                                             \
    return x;                                                                  \
  }

private:
  IDXGIFactory1 *factory1_ = nullptr;
  IDXGIAdapter1 *adapter1_ = nullptr;
  IDXGIAdapter *adapter_ = nullptr;
  /// DDA wrapper object, defined in DDAImpl.h
  DDAImpl *pDDAWrapper = nullptr;
  /// D3D11 device context used for the operations demonstrated in this
  /// application
  ID3D11Device *pD3DDev = nullptr;
  /// D3D11 device context
  ID3D11DeviceContext *pCtx = nullptr;
  /// D3D11 RGB Texture2D object that recieves the captured image from DDA
  ID3D11Texture2D *pDupTex2D = nullptr;
  /// D3D11 YUV420 Texture2D object that sends the image to NVENC for video
  /// encoding
  ID3D11Texture2D *pEncBuf = nullptr;
  ID3D10Multithread *hmt = NULL;
  int64_t m_luid = 0;

private:
  /// Initialize DXGI pipeline
  HRESULT InitDXGI() {
    HRESULT hr = S_OK;

    hr = CreateDXGIFactory1(__uuidof(IDXGIFactory1), (void **)&factory1_);
    if (FAILED(hr)) {
      return hr;
    }

    UINT i = 0;
    while (!FAILED(factory1_->EnumAdapters1(i, &adapter1_))) {
      i++;
      DXGI_ADAPTER_DESC1 desc = DXGI_ADAPTER_DESC1();
      adapter1_->GetDesc1(&desc);
      if ((((int64_t)desc.AdapterLuid.HighPart << 32) |
           desc.AdapterLuid.LowPart) == m_luid) {
        break;
      }
      SAFE_RELEASE(adapter1_);
    }
    if (!adapter1_) {
      return S_FALSE;
    }
    hr = adapter1_->QueryInterface(__uuidof(IDXGIAdapter), (void **)&adapter_);
    if (FAILED(hr)) {
      return hr;
    }

    /// Feature levels supported
    D3D_FEATURE_LEVEL FeatureLevels[] = {D3D_FEATURE_LEVEL_11_0};
    UINT NumFeatureLevels = ARRAYSIZE(FeatureLevels);
    D3D_FEATURE_LEVEL FeatureLevel = D3D_FEATURE_LEVEL_11_0;

    /// Create device
    hr = D3D11CreateDevice(adapter1_, D3D_DRIVER_TYPE_UNKNOWN, nullptr,
                           D3D11_CREATE_DEVICE_VIDEO_SUPPORT |
                               D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                           FeatureLevels, NumFeatureLevels, D3D11_SDK_VERSION,
                           &pD3DDev, &FeatureLevel, &pCtx);
    if (SUCCEEDED(hr)) {
      // Device creation succeeded, no need to loop anymore
      hr = pCtx->QueryInterface(IID_PPV_ARGS(&hmt));
      if (SUCCEEDED(hr)) {
        hr = hmt->SetMultithreadProtected(TRUE);
      }
    }
    return hr;
  }

  /// Initialize DDA handler
  HRESULT InitDup() {
    HRESULT hr = S_OK;
    if (!pDDAWrapper) {
      pDDAWrapper = new DDAImpl(pD3DDev, pCtx);
      hr = pDDAWrapper->Init();
      returnIfError(hr);
    }
    return hr;
  }

public:
  HRESULT Init() {
    HRESULT hr = S_OK;

    hr = InitDXGI();
    returnIfError(hr);

    hr = InitDup();
    returnIfError(hr);

    return hr;
  }

  ID3D11Device *Device() { return pD3DDev; }

  int width() { return pDDAWrapper->getWidth(); }

  int height() { return pDDAWrapper->getHeight(); }

  /// Capture a frame using DDA
  ID3D11Texture2D *Capture(int wait) {
    HRESULT hr = pDDAWrapper->GetCapturedFrame(&pDupTex2D,
                                               wait); // Release after preproc
    if (FAILED(hr)) {
      return NULL;
    }
    return pDupTex2D;
  }

  /// Release all resources
  void Cleanup(bool bDelete = true) {
    if (pDDAWrapper) {
      pDDAWrapper->Cleanup();
      delete pDDAWrapper;
      pDDAWrapper = nullptr;
    }

    SAFE_RELEASE(pDupTex2D);
    if (bDelete) {
      SAFE_RELEASE(factory1_);
      SAFE_RELEASE(adapter_);
      SAFE_RELEASE(adapter1_);
      SAFE_RELEASE(pD3DDev);
      SAFE_RELEASE(pCtx);
      SAFE_RELEASE(hmt)
    }
  }
  DemoApplication(int64_t luid) { m_luid = luid; }
  ~DemoApplication() { Cleanup(true); }
};

#endif // DDA_H