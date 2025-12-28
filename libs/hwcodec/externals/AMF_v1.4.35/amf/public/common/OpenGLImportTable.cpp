//
// Notice Regarding Standards.  AMD does not provide a license or sublicense to
// any Intellectual Property Rights relating to any standards, including but not
// limited to any audio and/or video codec technologies such as MPEG-2, MPEG-4;
// AVC/H.264; HEVC/H.265; AAC decode/FFMPEG; AAC encode/FFMPEG; VC-1; and MP3
// (collectively, the "Media Technologies"). For clarity, you will pay any
// royalties due for such third party technologies, which may include the Media
// Technologies that are owed as a result of AMD providing the Software to you.
//
// MIT license
//
// Copyright (c) 2018 Advanced Micro Devices, Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
//
///-------------------------------------------------------------------------
///  @file   OpenGLImportTable.cpp
///  @brief  OpenGL import table
///-------------------------------------------------------------------------
#include "OpenGLImportTable.h"
#include "public/common/TraceAdapter.h"
#include "public/common/Thread.h"

using namespace amf;

#define AMF_FACILITY L"OpenGLImportTable"

//-------------------------------------------------------------------------------------------------


#define TRY_GET_DLL_ENTRY_POINT_CORE(w) \
w = reinterpret_cast<w##_fn>(amf_get_proc_address(m_hOpenGLDll, #w));

#define GET_DLL_ENTRY_POINT_CORE(w)\
TRY_GET_DLL_ENTRY_POINT_CORE(w)\
AMF_RETURN_IF_FALSE(w != nullptr, AMF_NOT_FOUND, L"Failed to aquire entry point %S", #w);

// On windows, some functions are defined in the core opengl32.dll (especially the core old ones)
// and some are not and we have to use wglGetProcAddress. Its a problem because the ones defined in
// opengl32.dll are not included in the wglGetProcAddress and vice versa
#if defined(_WIN32)

#define TRY_GET_DLL_ENTRY_POINT(w) \
{\
    void const * const p = (void*)wglGetProcAddress(#w);\
    if (p == nullptr || p == (void*)0x1 || p == (void*)0x2 || p == (void*)0x3 || p == (void*)-1)\
    {\
        TRY_GET_DLL_ENTRY_POINT_CORE(w);\
    }\
    else\
    {\
        w = reinterpret_cast<w##_fn>(p);\
    }\
}

#else
#define TRY_GET_DLL_ENTRY_POINT(w)   TRY_GET_DLL_ENTRY_POINT_CORE(w)
#endif

#define GET_DLL_ENTRY_POINT(w)\
TRY_GET_DLL_ENTRY_POINT(w)\
AMF_RETURN_IF_FALSE(w != nullptr, AMF_NOT_FOUND, L"Failed to aquire entry point %S", #w);

OpenGLImportTable::OpenGLImportTable() :
    m_hOpenGLDll(nullptr),
    glGetError(nullptr),
    glGetString(nullptr),
//    glGetStringi(nullptr),
    glEnable(nullptr),
    glClear(nullptr),
    glClearAccum(nullptr),
    glClearColor(nullptr),
    glClearDepth(nullptr),
    glClearIndex(nullptr),
    glClearStencil(nullptr),
    glDrawArrays(nullptr),
    glViewport(nullptr),
    glFinish(nullptr),
#if defined(_WIN32)
    wglCreateContext(nullptr),
    wglDeleteContext(nullptr),
    wglGetCurrentContext(nullptr),
    wglGetCurrentDC(nullptr),
    wglMakeCurrent(nullptr),
    wglGetProcAddress(nullptr),
    wglGetExtensionsStringARB(nullptr),
    wglSwapIntervalEXT(nullptr),

    wndClass{},
    hDummyWnd(nullptr),
    hDummyDC(nullptr),
    hDummyOGLContext(nullptr),
#elif defined(__ANDROID__)
    eglInitialize(nullptr),
    eglGetDisplay(nullptr),
    eglChooseConfig(nullptr),
    eglCreateContext(nullptr),
    eglDestroyImageKHR(nullptr),
    eglCreateImageKHR(nullptr),
    eglSwapInterval(nullptr),
    glEGLImageTargetTexture2DOES(nullptr),
    glReadPixels(nullptr),
#elif defined(__linux)
    glXDestroyContext(nullptr),
    glXDestroyWindow(nullptr),
    glXSwapBuffers(nullptr),
    glXQueryExtension(nullptr),
    glXChooseFBConfig(nullptr),
    glXCreateWindow(nullptr),
    glXCreateNewContext(nullptr),
    glXMakeCurrent(nullptr),
    glXGetCurrentContext(nullptr),
    glXGetCurrentDrawable(nullptr),
    glXQueryExtensionsString(nullptr),
    glXSwapIntervalEXT(nullptr),
#endif

    glBindTexture(nullptr),
    glDeleteTextures(nullptr),
    glGenTextures(nullptr),
    glGetTexImage(nullptr),
    glGetTexLevelParameteriv(nullptr),
    glTexParameteri(nullptr),
    glTexImage2D(nullptr),
    glActiveTexture(nullptr),

    glBindFramebuffer(nullptr),
//    glBindRenderbuffer(nullptr),
    glBlitFramebuffer(nullptr),
    glCheckFramebufferStatus(nullptr),
    glDeleteFramebuffers(nullptr),
//    glDeleteRenderbuffers(nullptr),
//    glFramebufferRenderbuffer(nullptr),
//    glFramebufferTexture1D(nullptr),
    glFramebufferTexture2D(nullptr),
//    glFramebufferTexture3D(nullptr),
    glFramebufferTextureLayer(nullptr),
    glGenFramebuffers(nullptr),
//    glGenRenderbuffers(nullptr),
//    glGenerateMipmap(nullptr),
//    glGetFramebufferAttachmentParameteriv(nullptr),
//    glGetRenderbufferParameteriv(nullptr),
//    glIsFramebuffer(nullptr),
//    glIsRenderbuffer(nullptr),
//    glRenderbufferStorage(nullptr),
//    glRenderbufferStorageMultisample(nullptr),

    glGenBuffers(nullptr),
    glBindBuffer(nullptr),
    glBufferData(nullptr),
    glBufferSubData(nullptr),
    glDeleteBuffers(nullptr),

    glVertexAttribPointer(nullptr),
//    glVertexAttribLPointer(nullptr),
//    glVertexAttribIPointer(nullptr),
    glBindVertexBuffer(nullptr),
    glDisableVertexAttribArray(nullptr),
    glEnableVertexAttribArray(nullptr),

    glBindVertexArray(nullptr),
    glDeleteVertexArrays(nullptr),
    glGenVertexArrays(nullptr),
    glIsVertexArray(nullptr),

    glCreateShader(nullptr),
    glShaderSource(nullptr),
    glCompileShader(nullptr),
    glGetShaderInfoLog(nullptr),
    glGetShaderSource(nullptr),
    glGetShaderiv(nullptr),
    glCreateProgram(nullptr),
    glAttachShader(nullptr),
    glLinkProgram(nullptr),
    glGetProgramInfoLog(nullptr),
    glGetProgramiv(nullptr),
    glValidateProgram(nullptr),
    glUseProgram(nullptr),
    glDeleteShader(nullptr),
    glDeleteProgram(nullptr),

    glGetUniformLocation(nullptr),
//    glUniform1f(nullptr),
//    glUniform1fv(nullptr),
    glUniform1i(nullptr),
//    glUniform1iv(nullptr),
//    glUniform2f(nullptr),
//    glUniform2fv(nullptr),
//    glUniform2i(nullptr),
//    glUniform2iv(nullptr),
//    glUniform3f(nullptr),
//    glUniform3fv(nullptr),
//    glUniform3i(nullptr),
//    glUniform3iv(nullptr),
//    glUniform4f(nullptr),
    glUniform4fv(nullptr),
//    glUniform4i(nullptr),
//    glUniform4iv(nullptr),
//    glUniformMatrix2fv(nullptr),
//    glUniformMatrix3fv(nullptr),
//    glUniformMatrix4fv(nullptr),

    glBindBufferBase(nullptr),
    glBindBufferRange(nullptr),
    glGetUniformBlockIndex(nullptr),
    glUniformBlockBinding(nullptr),

    glBindSampler(nullptr),
    glDeleteSamplers(nullptr),
    glGenSamplers(nullptr),
//    glGetSamplerParameterIiv(nullptr),
//    glGetSamplerParameterIuiv(nullptr),
//    glGetSamplerParameterfv(nullptr),
//    glGetSamplerParameteriv(nullptr),
//    glIsSampler(nullptr),
//    glSamplerParameterIiv(nullptr),
//    glSamplerParameterIuiv(nullptr),
    glSamplerParameterf(nullptr),
    glSamplerParameterfv(nullptr),
    glSamplerParameteri(nullptr)
//    glSamplerParameteriv(nullptr)

{
}

OpenGLImportTable::~OpenGLImportTable()
{
    if (m_hOpenGLDll != nullptr)
    {
        amf_free_library(m_hOpenGLDll);
    }
    m_hOpenGLDll = nullptr;

#if defined(_WIN32)
    DestroyDummy();
#endif
}

AMF_RESULT OpenGLImportTable::LoadFunctionsTable()
{
    if (m_hOpenGLDll != nullptr)
    {
        return AMF_OK;
    }
#if defined(_WIN32)
    m_hOpenGLDll = amf_load_library(L"opengl32.dll");
#elif defined(__ANDROID__)
    m_hOpenGLDll = amf_load_library1(L"libGLES.so", true);
#elif defined(__linux__)
    m_hOpenGLDll = amf_load_library1(L"libGL.so.1", true);
#endif

    if (m_hOpenGLDll == nullptr)
    {
        AMFTraceError(L"OpenGLImportTable", L"amf_load_library() failed to load opengl dll!");
        return AMF_FAIL;
    }

    // Core
    GET_DLL_ENTRY_POINT_CORE(glGetError);
    GET_DLL_ENTRY_POINT_CORE(glGetString);

    GET_DLL_ENTRY_POINT_CORE(glEnable);
    GET_DLL_ENTRY_POINT_CORE(glClear);
    GET_DLL_ENTRY_POINT_CORE(glClearAccum);
    GET_DLL_ENTRY_POINT_CORE(glClearColor);
    GET_DLL_ENTRY_POINT_CORE(glClearDepth);
    GET_DLL_ENTRY_POINT_CORE(glClearIndex);
    GET_DLL_ENTRY_POINT_CORE(glClearStencil);
    GET_DLL_ENTRY_POINT_CORE(glDrawArrays);
    GET_DLL_ENTRY_POINT_CORE(glViewport);
    GET_DLL_ENTRY_POINT_CORE(glFinish);

    // Core (platform-dependent)
#if defined(_WIN32)
    GET_DLL_ENTRY_POINT_CORE(wglCreateContext);
    GET_DLL_ENTRY_POINT_CORE(wglDeleteContext);
    GET_DLL_ENTRY_POINT_CORE(wglGetCurrentContext);
    GET_DLL_ENTRY_POINT_CORE(wglGetCurrentDC);
    GET_DLL_ENTRY_POINT_CORE(wglMakeCurrent);
    GET_DLL_ENTRY_POINT_CORE(wglGetProcAddress);
#elif defined(__ANDROID__)
    GET_DLL_ENTRY_POINT_CORE(eglInitialize);
    GET_DLL_ENTRY_POINT_CORE(eglGetDisplay);
    GET_DLL_ENTRY_POINT_CORE(eglChooseConfig);
    GET_DLL_ENTRY_POINT_CORE(eglCreateContext);
    GET_DLL_ENTRY_POINT_CORE(eglDestroyImageKHR);
    GET_DLL_ENTRY_POINT_CORE(eglCreateImageKHR);
    GET_DLL_ENTRY_POINT_CORE(glEGLImageTargetTexture2DOES);
    GET_DLL_ENTRY_POINT_CORE(glReadPixels);
#elif defined(__linux)
    GET_DLL_ENTRY_POINT_CORE(glXDestroyContext);
    GET_DLL_ENTRY_POINT_CORE(glXDestroyWindow);
    GET_DLL_ENTRY_POINT_CORE(glXSwapBuffers);
    GET_DLL_ENTRY_POINT_CORE(glXQueryExtension);
    GET_DLL_ENTRY_POINT_CORE(glXChooseFBConfig);
    GET_DLL_ENTRY_POINT_CORE(glXCreateWindow);
    GET_DLL_ENTRY_POINT_CORE(glXCreateNewContext);
    GET_DLL_ENTRY_POINT_CORE(glXMakeCurrent);
    GET_DLL_ENTRY_POINT_CORE(glXGetCurrentContext);
    GET_DLL_ENTRY_POINT_CORE(glXGetCurrentDrawable);
#endif

    // Textures
    GET_DLL_ENTRY_POINT_CORE(glBindTexture);
    GET_DLL_ENTRY_POINT_CORE(glDeleteTextures);
    GET_DLL_ENTRY_POINT_CORE(glGenTextures);
    GET_DLL_ENTRY_POINT_CORE(glGetTexImage);
    GET_DLL_ENTRY_POINT_CORE(glGetTexLevelParameteriv);
    GET_DLL_ENTRY_POINT_CORE(glTexParameteri);
    GET_DLL_ENTRY_POINT_CORE(glTexImage2D);

    // For windows, we need to use wglGetProcAddress to get some
    // addresses however that requires a context. We can just create
    // a small dummy context/window and then delete it when we are done
#if defined(_WIN32)
    {
        AMF_RESULT res = CreateDummy();
        if (res != AMF_OK)
        {
            DestroyDummy();
            AMF_RETURN_IF_FAILED(res, L"CreateDummy() failed");
        }
    }
#endif

    AMF_RESULT res = LoadContextFunctionsTable();
    AMF_RETURN_IF_FAILED(res, L"LoadContextFunctionsTable() failed");

#if defined(_WIN32)
    DestroyDummy();
#endif

    return AMF_OK;
}

AMF_RESULT OpenGLImportTable::LoadContextFunctionsTable()
{
    if (m_hOpenGLDll == nullptr)
    {
        AMF_RETURN_IF_FAILED(LoadFunctionsTable());
    }

#if defined(_WIN32)
    HGLRC context = wglGetCurrentContext();
    AMF_RETURN_IF_FALSE(context != nullptr, AMF_NOT_INITIALIZED, L"LoadContextFunctionsTable() - context is not initialized");
#endif

    // Core
//    GET_DLL_ENTRY_POINT(glGetStringi);

#if defined(_WIN32)
    TRY_GET_DLL_ENTRY_POINT(wglGetExtensionsStringARB);
    TRY_GET_DLL_ENTRY_POINT(wglSwapIntervalEXT);
#elif defined(__ANDROID__)
    TRY_GET_DLL_ENTRY_POINT(eglSwapInterval);
#elif defined(__linux)
    TRY_GET_DLL_ENTRY_POINT(glXQueryExtensionsString);
    TRY_GET_DLL_ENTRY_POINT(glXSwapIntervalEXT);
#endif

    // Textures
    GET_DLL_ENTRY_POINT(glActiveTexture);

    // Frame buffer and render buffer objects
    GET_DLL_ENTRY_POINT(glBindFramebuffer);
//    GET_DLL_ENTRY_POINT(glBindRenderbuffer);
    GET_DLL_ENTRY_POINT(glBlitFramebuffer);
    GET_DLL_ENTRY_POINT(glCheckFramebufferStatus);
    GET_DLL_ENTRY_POINT(glDeleteFramebuffers);
//    GET_DLL_ENTRY_POINT(glDeleteRenderbuffers);
//    GET_DLL_ENTRY_POINT(glFramebufferRenderbuffer);
//    GET_DLL_ENTRY_POINT(glFramebufferTexture1D);
    GET_DLL_ENTRY_POINT(glFramebufferTexture2D);
//    GET_DLL_ENTRY_POINT(glFramebufferTexture3D);
    GET_DLL_ENTRY_POINT(glFramebufferTextureLayer);
    GET_DLL_ENTRY_POINT(glGenFramebuffers);
//    GET_DLL_ENTRY_POINT(glGenRenderbuffers);
//    GET_DLL_ENTRY_POINT(glGenerateMipmap);
//    GET_DLL_ENTRY_POINT(glGetFramebufferAttachmentParameteriv);
//    GET_DLL_ENTRY_POINT(glGetRenderbufferParameteriv);
//    GET_DLL_ENTRY_POINT(glIsFramebuffer);
//    GET_DLL_ENTRY_POINT(glIsRenderbuffer);
//    GET_DLL_ENTRY_POINT(glRenderbufferStorage);
//    GET_DLL_ENTRY_POINT(glRenderbufferStorageMultisample);

    // Buffers
    GET_DLL_ENTRY_POINT(glGenBuffers);
    GET_DLL_ENTRY_POINT(glBindBuffer);
    GET_DLL_ENTRY_POINT(glBufferData);
    GET_DLL_ENTRY_POINT(glBufferSubData);
    GET_DLL_ENTRY_POINT(glDeleteBuffers);

    // Vertex buffer attributes
    GET_DLL_ENTRY_POINT(glVertexAttribPointer);
//    GET_DLL_ENTRY_POINT(glVertexAttribLPointer);
//    GET_DLL_ENTRY_POINT(glVertexAttribIPointer);
    GET_DLL_ENTRY_POINT(glBindVertexBuffer);
    GET_DLL_ENTRY_POINT(glDisableVertexAttribArray);
    GET_DLL_ENTRY_POINT(glEnableVertexAttribArray);

    GET_DLL_ENTRY_POINT(glBindVertexArray);
    GET_DLL_ENTRY_POINT(glDeleteVertexArrays);
    GET_DLL_ENTRY_POINT(glGenVertexArrays);
    GET_DLL_ENTRY_POINT(glIsVertexArray);

    // Shaders
    GET_DLL_ENTRY_POINT(glCreateShader);
    GET_DLL_ENTRY_POINT(glShaderSource);
    GET_DLL_ENTRY_POINT(glCompileShader);
    GET_DLL_ENTRY_POINT(glGetShaderInfoLog);
    GET_DLL_ENTRY_POINT(glGetShaderSource);
    GET_DLL_ENTRY_POINT(glGetShaderiv);
    GET_DLL_ENTRY_POINT(glCreateProgram);
    GET_DLL_ENTRY_POINT(glAttachShader);
    GET_DLL_ENTRY_POINT(glLinkProgram);
    GET_DLL_ENTRY_POINT(glGetProgramInfoLog);
    GET_DLL_ENTRY_POINT(glGetProgramiv);
    GET_DLL_ENTRY_POINT(glValidateProgram);
    GET_DLL_ENTRY_POINT(glUseProgram);
    GET_DLL_ENTRY_POINT(glDeleteShader);
    GET_DLL_ENTRY_POINT(glDeleteProgram);

    // Uniforms
    GET_DLL_ENTRY_POINT(glGetUniformLocation);
//    GET_DLL_ENTRY_POINT(glUniform1f);
//    GET_DLL_ENTRY_POINT(glUniform1fv);
    GET_DLL_ENTRY_POINT(glUniform1i);
//    GET_DLL_ENTRY_POINT(glUniform1iv);
//    GET_DLL_ENTRY_POINT(glUniform2f);
//    GET_DLL_ENTRY_POINT(glUniform2fv);
//    GET_DLL_ENTRY_POINT(glUniform2i);
//    GET_DLL_ENTRY_POINT(glUniform2iv);
//    GET_DLL_ENTRY_POINT(glUniform3f);
//    GET_DLL_ENTRY_POINT(glUniform3fv);
//    GET_DLL_ENTRY_POINT(glUniform3i);
//    GET_DLL_ENTRY_POINT(glUniform3iv);
//    GET_DLL_ENTRY_POINT(glUniform4f);
    GET_DLL_ENTRY_POINT(glUniform4fv);
//    GET_DLL_ENTRY_POINT(glUniform4i);
//    GET_DLL_ENTRY_POINT(glUniform4iv);
//    GET_DLL_ENTRY_POINT(glUniformMatrix2fv);
//    GET_DLL_ENTRY_POINT(glUniformMatrix3fv);
//    GET_DLL_ENTRY_POINT(glUniformMatrix4fv);

    // Uniform buffer objects
    GET_DLL_ENTRY_POINT(glBindBufferBase);
    GET_DLL_ENTRY_POINT(glBindBufferRange);
    GET_DLL_ENTRY_POINT(glGetUniformBlockIndex);
    GET_DLL_ENTRY_POINT(glUniformBlockBinding);

    // Sampler objects
    GET_DLL_ENTRY_POINT(glBindSampler);
    GET_DLL_ENTRY_POINT(glDeleteSamplers);
    GET_DLL_ENTRY_POINT(glGenSamplers);
//    GET_DLL_ENTRY_POINT(glGetSamplerParameterIiv);
//    GET_DLL_ENTRY_POINT(glGetSamplerParameterIuiv);
//    GET_DLL_ENTRY_POINT(glGetSamplerParameterfv);
//    GET_DLL_ENTRY_POINT(glGetSamplerParameteriv);
//    GET_DLL_ENTRY_POINT(glIsSampler);
//    GET_DLL_ENTRY_POINT(glSamplerParameterIiv);
//    GET_DLL_ENTRY_POINT(glSamplerParameterIuiv);
    GET_DLL_ENTRY_POINT(glSamplerParameterf);
    GET_DLL_ENTRY_POINT(glSamplerParameterfv);
    GET_DLL_ENTRY_POINT(glSamplerParameteri);
//    GET_DLL_ENTRY_POINT(glSamplerParameteriv);

    return AMF_OK;
}

#if defined(_WIN32)
AMF_RESULT OpenGLImportTable::CreateDummy()
{
    DestroyDummy();

    wndClass = { 0 };
    wndClass.cbSize = sizeof(wndClass);
    wndClass.style = CS_HREDRAW | CS_VREDRAW | CS_OWNDC;
    wndClass.lpfnWndProc = DefWindowProcW;
    wndClass.hInstance = GetModuleHandle(0);
    wndClass.lpszClassName = L"OpenGL_Dummy_Class";

    int ret = RegisterClassExW(&wndClass);
    AMF_RETURN_IF_FALSE(ret != 0, AMF_FAIL, L"CreateDummy() - RegisterClassA() failed, error=%d", GetLastError());

    hDummyWnd = CreateWindowExW(0, wndClass.lpszClassName, L"Dummy OpenGL Window", 0, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, 0, 0, wndClass.hInstance, 0);
    AMF_RETURN_IF_FALSE(hDummyWnd != nullptr, AMF_FAIL, L"CreateDummy() - CreateWindowExA() failed to create window");

    hDummyDC = GetDC(hDummyWnd);

    PIXELFORMATDESCRIPTOR pfd = {};
    pfd.nSize = sizeof(pfd);
    pfd.nVersion = 1;
    pfd.iPixelType = PFD_TYPE_RGBA;
    pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
    pfd.cColorBits = 32;
    pfd.cAlphaBits = 8;
    pfd.iLayerType = PFD_MAIN_PLANE;
    pfd.cDepthBits = 24;
    pfd.cStencilBits = 8;

    int pixel_format = ChoosePixelFormat(hDummyDC, &pfd);
    AMF_RETURN_IF_FALSE(pixel_format != 0, AMF_FAIL, L"CreateDummy() - ChoosePixelFormat() failed to find a suitable pixel format.");

    ret = SetPixelFormat(hDummyDC, pixel_format, &pfd);
    AMF_RETURN_IF_FALSE(ret != 0, AMF_FAIL, L"CreateDummy() - SetPixelFormat() failed");

    hDummyOGLContext = wglCreateContext(hDummyDC);
    AMF_RETURN_IF_FALSE(hDummyOGLContext != nullptr, AMF_FAIL, L"CreateDummy() - wglCreateContext() failed");

    ret = !wglMakeCurrent(hDummyDC, hDummyOGLContext);
    AMF_RETURN_IF_FALSE(hDummyOGLContext != nullptr, AMF_FAIL, L"CreateDummy() - wglMakeCurrent() failed");

    return AMF_OK;
}


AMF_RESULT OpenGLImportTable::DestroyDummy()
{
    if (hDummyOGLContext != nullptr)
    {
        wglDeleteContext(hDummyOGLContext);
        hDummyOGLContext = nullptr;
    }

    if (hDummyWnd != nullptr || hDummyDC != nullptr)
    {
        if (wglMakeCurrent != nullptr)
        {
            wglMakeCurrent(hDummyDC, 0);
        }

        ReleaseDC(hDummyWnd, hDummyDC);
        DestroyWindow(hDummyWnd);
        hDummyWnd = nullptr;
        hDummyDC = nullptr;
    }

    if (wndClass.lpszClassName != nullptr)
    {
        UnregisterClassW(wndClass.lpszClassName, wndClass.hInstance);
        wndClass = {};
    }

    return AMF_OK;
}
#endif