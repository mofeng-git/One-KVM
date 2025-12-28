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
///  @file   OpenGLImportTable.h
///  @brief  OpenGL import table
///-------------------------------------------------------------------------
#pragma once

#include "public/include/core/Result.h"
#include "public/common/AMFSTL.h"

#if defined(_WIN32)
#include <Wingdi.h>
#include <gl/GL.h>
#include <gl/GLU.h>
#elif defined(__ANDROID__)
////todo:AA    #include <android/native_window.h> // requires ndk r5 or newer
#define GL_GLEXT_PROTOTYPES
#define EGL_EGLEXT_PROTOTYPES

#include <EGL/egl.h> // requires ndk r5 or newer
#include <EGL/eglext.h>
#include <GLES/gl.h> // requires ndk r5 or newer
#include <GLES/glext.h> // requires ndk r5 or newer
#include <GLES2/gl2.h>
#include <GLES2/gl2ext.h>
////todo:AA    #include <ui/FramebufferNativeWindow.h>
//    #include <gralloc_priv.h>
#include <time.h>
#if !defined(CLOCK_MONOTONIC_RAW)
#define CLOCK_MONOTONIC_RAW 4
#endif
////todo:AA    #include <amdgralloc.h>

////todo:AA    using namespace android;
#if !defined(GL_CLAMP)
#define GL_CLAMP GL_CLAMP_TO_EDGE
#endif

#elif defined(__linux)
#include <GL/glx.h>
#include <GL/glu.h>
#endif

#ifndef AMF_GLAPI
    #if defined(_WIN32)
        #define AMF_GLAPI WINGDIAPI
    #elif defined(__ANDROID__)
        #define AMF_GLAPI GL_API
    #else // __linux
        #define AMF_GLAPI
    #endif
#endif

#ifndef AMF_GLAPIENTRY
    #if defined(_WIN32)
        #define AMF_GLAPIENTRY APIENTRY
    #elif defined(__ANDROID__)
        #define AMF_GLAPIENTRY GL_APIENTRY
    #elif defined(__linux)
        #define AMF_GLAPIENTRY GLAPIENTRY
    #else
        #define AMF_GLAPIENTRY
    #endif
#endif


typedef char GLchar;
#if defined(__ANDROID__)
typedef double GLclampd;
#define GL_TEXTURE_BORDER_COLOR           0x1004
#else
typedef ptrdiff_t GLintptr;
#endif

#ifdef _WIN32
typedef size_t GLsizeiptr; // Defined in glx.h on linux
#endif


// Core
typedef AMF_GLAPI GLenum            (AMF_GLAPIENTRY* glGetError_fn)                             (void);
typedef AMF_GLAPI const GLubyte*    (AMF_GLAPIENTRY* glGetString_fn)                            (GLenum name);
typedef AMF_GLAPI const GLubyte*    (AMF_GLAPIENTRY* glGetStringi_fn)                           (GLenum name, GLuint index);

typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glEnable_fn)                               (GLenum cap);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glClear_fn)                                (GLbitfield mask);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glClearAccum_fn)                           (GLfloat red, GLfloat green, GLfloat blue, GLfloat alpha);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glClearColor_fn)                           (GLclampf red, GLclampf green, GLclampf blue, GLclampf alpha);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glClearDepth_fn)                           (GLclampd depth);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glClearIndex_fn)                           (GLfloat c);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glClearStencil_fn)                         (GLint s);

typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glDrawArrays_fn)                           (GLenum mode, GLint first, GLsizei count);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glViewport_fn)                             (GLint x, GLint y, GLsizei width, GLsizei height);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glFinish_fn)                               (void);

// Textures
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBindTexture_fn)                          (GLenum target, GLuint texture);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glDeleteTextures_fn)                       (GLsizei n, const GLuint* textures);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGenTextures_fn)                          (GLsizei n, GLuint* textures);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetTexImage_fn)                          (GLenum target, GLint level, GLenum format, GLenum type, GLvoid* pixels);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetTexLevelParameteriv_fn)               (GLenum target, GLint level, GLenum pname, GLint* params);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glTexParameteri_fn)                        (GLenum target, GLenum pname, GLint param);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glTexImage2D_fn)                           (GLenum target, GLint level, GLint internalformat, GLsizei width, GLsizei height,
                                                                                                 GLint border, GLenum format, GLenum type, const GLvoid* pixels);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glActiveTexture_fn)                        (GLenum texture);

// Framebuffer and Renderbuffer objects - EXT: GL_ARB_framebuffer_object
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBindFramebuffer_fn)                      (GLenum target, GLuint framebuffer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBindRenderbuffer_fn)                     (GLenum target, GLuint renderbuffer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBlitFramebuffer_fn)                      (GLint srcX0, GLint srcY0, GLint srcX1, GLint srcY1, GLint dstX0, GLint dstY0, GLint dstX1, GLint dstY1, GLbitfield mask, GLenum filter);
typedef AMF_GLAPI GLenum            (AMF_GLAPIENTRY* glCheckFramebufferStatus_fn)               (GLenum target);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glDeleteFramebuffers_fn)                   (GLsizei n, const GLuint* framebuffers);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glDeleteRenderbuffers_fn)                  (GLsizei n, const GLuint* renderbuffers);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glFramebufferRenderbuffer_fn)              (GLenum target, GLenum attachment, GLenum renderbuffertarget, GLuint renderbuffer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glFramebufferTexture1D_fn)                 (GLenum target, GLenum attachment, GLenum textarget, GLuint texture, GLint level);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glFramebufferTexture2D_fn)                 (GLenum target, GLenum attachment, GLenum textarget, GLuint texture, GLint level);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glFramebufferTexture3D_fn)                 (GLenum target, GLenum attachment, GLenum textarget, GLuint texture, GLint level, GLint layer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glFramebufferTextureLayer_fn)              (GLenum target, GLenum attachment, GLuint texture, GLint level, GLint layer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGenFramebuffers_fn)                      (GLsizei n, GLuint* framebuffers);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGenRenderbuffers_fn)                     (GLsizei n, GLuint* renderbuffers);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGenerateMipmap_fn)                       (GLenum target);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetFramebufferAttachmentParameteriv_fn)  (GLenum target, GLenum attachment, GLenum pname, GLint* params);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetRenderbufferParameteriv_fn)           (GLenum target, GLenum pname, GLint* params);
typedef AMF_GLAPI GLboolean         (AMF_GLAPIENTRY* glIsFramebuffer_fn)                        (GLuint framebuffer);
typedef AMF_GLAPI GLboolean         (AMF_GLAPIENTRY* glIsRenderbuffer_fn)                       (GLuint renderbuffer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glRenderbufferStorage_fn)                  (GLenum target, GLenum internalformat, GLsizei width, GLsizei height);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glRenderbufferStorageMultisample_fn)       (GLenum target, GLsizei samples, GLenum internalformat, GLsizei width, GLsizei height);

// Buffers
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGenBuffers_fn)                           (GLsizei n, GLuint* buffers);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBindBuffer_fn)                           (GLenum target, GLuint buffer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBufferData_fn)                           (GLenum target, GLsizeiptr size, const void* data, GLenum usage);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBufferSubData_fn)                        (GLenum target, GLintptr offset, GLsizeiptr size, const void* data);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glDeleteBuffers_fn)                        (GLsizei n, const GLuint* buffers);

// Vertex attributes
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glVertexAttribPointer_fn)                  (GLuint index, GLint size, GLenum type, GLboolean normalized, GLsizei stride, const void* pointer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glVertexAttribLPointer_fn)                 (GLuint index, GLint size, GLenum type, GLsizei stride, const void* pointer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glVertexAttribIPointer_fn)                 (GLuint index, GLint size, GLenum type, GLsizei stride, const void* pointer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBindVertexBuffer_fn)                     (GLuint bindingindex, GLuint buffer, GLintptr offset, GLsizei stride);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glDisableVertexAttribArray_fn)             (GLuint index);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glEnableVertexAttribArray_fn)              (GLuint index);

// Vertex array objects
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBindVertexArray_fn)                      (GLuint array);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glDeleteVertexArrays_fn)                   (GLsizei n, const GLuint* arrays);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGenVertexArrays_fn)                      (GLsizei n, GLuint* arrays);
typedef AMF_GLAPI GLboolean         (AMF_GLAPIENTRY* glIsVertexArray_fn)                        (GLuint array);

// Shaders
typedef AMF_GLAPI GLuint            (AMF_GLAPIENTRY* glCreateShader_fn)                         (GLenum type);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glShaderSource_fn)                         (GLuint shader, GLsizei count, const GLchar* const* string, const GLint* length);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glCompileShader_fn)                        (GLuint shader);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetShaderInfoLog_fn)                     (GLuint shader, GLsizei bufSize, GLsizei* length, GLchar* infoLog);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetShaderSource_fn)                      (GLuint obj, GLsizei maxLength, GLsizei* length, GLchar* source);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetShaderiv_fn)                          (GLuint shader, GLenum pname, GLint* param);
typedef AMF_GLAPI GLuint            (AMF_GLAPIENTRY* glCreateProgram_fn)                        (void);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glAttachShader_fn)                         (GLuint program, GLuint shader);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glLinkProgram_fn)                          (GLuint program);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetProgramInfoLog_fn)                    (GLuint program, GLsizei bufSize, GLsizei* length, GLchar* infoLog);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetProgramiv_fn)                         (GLuint program, GLenum pname, GLint* param);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glValidateProgram_fn)                      (GLuint program);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUseProgram_fn)                           (GLuint program);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glDeleteShader_fn)                         (GLuint shader);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glDeleteProgram_fn)                        (GLuint program);

// Uniforms
typedef AMF_GLAPI GLint             (AMF_GLAPIENTRY* glGetUniformLocation_fn)                   (GLuint program, const GLchar* name);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform1f_fn)                            (GLint location, GLfloat v0);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform1fv_fn)                           (GLint location, GLsizei count, const GLfloat* value);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform1i_fn)                            (GLint location, GLint v0);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform1iv_fn)                           (GLint location, GLsizei count, const GLint* value);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform2f_fn)                            (GLint location, GLfloat v0, GLfloat v1);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform2fv_fn)                           (GLint location, GLsizei count, const GLfloat* value);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform2i_fn)                            (GLint location, GLint v0, GLint v1);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform2iv_fn)                           (GLint location, GLsizei count, const GLint* value);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform3f_fn)                            (GLint location, GLfloat v0, GLfloat v1, GLfloat v2);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform3fv_fn)                           (GLint location, GLsizei count, const GLfloat* value);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform3i_fn)                            (GLint location, GLint v0, GLint v1, GLint v2);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform3iv_fn)                           (GLint location, GLsizei count, const GLint* value);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform4f_fn)                            (GLint location, GLfloat v0, GLfloat v1, GLfloat v2, GLfloat v3);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform4fv_fn)                           (GLint location, GLsizei count, const GLfloat* value);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform4i_fn)                            (GLint location, GLint v0, GLint v1, GLint v2, GLint v3);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniform4iv_fn)                           (GLint location, GLsizei count, const GLint* value);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniformMatrix2fv_fn)                     (GLint location, GLsizei count, GLboolean transpose, const GLfloat* value);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniformMatrix3fv_fn)                     (GLint location, GLsizei count, GLboolean transpose, const GLfloat* value);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniformMatrix4fv_fn)                     (GLint location, GLsizei count, GLboolean transpose, const GLfloat* value);

// Uniform block objects - EXT: ARB_Uniform_Buffer_Object
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBindBufferBase_fn)                       (GLenum target, GLuint index, GLuint buffer);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBindBufferRange_fn)                      (GLenum target, GLuint index, GLuint buffer, GLintptr offset, GLsizeiptr size);
typedef AMF_GLAPI GLuint            (AMF_GLAPIENTRY* glGetUniformBlockIndex_fn)                 (GLuint program, const GLchar* uniformBlockName);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glUniformBlockBinding_fn)                  (GLuint program, GLuint uniformBlockIndex, GLuint uniformBlockBinding);


// Sampler Objects - EXT: GL_ARB_sampler_objects
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glBindSampler_fn)                          (GLuint unit, GLuint sampler);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glDeleteSamplers_fn)                       (GLsizei count, const GLuint* samplers);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGenSamplers_fn)                          (GLsizei count, GLuint* samplers);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetSamplerParameterIiv_fn)               (GLuint sampler, GLenum pname, GLint* params);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetSamplerParameterIuiv_fn)              (GLuint sampler, GLenum pname, GLuint* params);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetSamplerParameterfv_fn)                (GLuint sampler, GLenum pname, GLfloat* params);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glGetSamplerParameteriv_fn)                (GLuint sampler, GLenum pname, GLint* params);
typedef AMF_GLAPI GLboolean         (AMF_GLAPIENTRY* glIsSampler_fn)                            (GLuint sampler);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glSamplerParameterIiv_fn)                  (GLuint sampler, GLenum pname, const GLint* params);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glSamplerParameterIuiv_fn)                 (GLuint sampler, GLenum pname, const GLuint* params);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glSamplerParameterf_fn)                    (GLuint sampler, GLenum pname, GLfloat param);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glSamplerParameterfv_fn)                   (GLuint sampler, GLenum pname, const GLfloat* params);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glSamplerParameteri_fn)                    (GLuint sampler, GLenum pname, GLint param);
typedef AMF_GLAPI void              (AMF_GLAPIENTRY* glSamplerParameteriv_fn)                   (GLuint sampler, GLenum pname, const GLint* params);


#if defined(_WIN32)
typedef WINGDIAPI HGLRC             (WINAPI* wglCreateContext_fn)                               (HDC);
typedef WINGDIAPI BOOL              (WINAPI* wglDeleteContext_fn)                               (HGLRC);
typedef WINGDIAPI HGLRC             (WINAPI* wglGetCurrentContext_fn)                           (VOID);
typedef WINGDIAPI HDC               (WINAPI* wglGetCurrentDC_fn)                                (VOID);
typedef WINGDIAPI BOOL              (WINAPI* wglMakeCurrent_fn)                                 (HDC, HGLRC);
typedef WINGDIAPI PROC              (WINAPI* wglGetProcAddress_fn)                              (LPCSTR func);
typedef WINGDIAPI const char*       (WINAPI* wglGetExtensionsStringARB_fn)                      (HDC hdc);
typedef WINGDIAPI BOOL              (WINAPI* wglSwapIntervalEXT_fn)                             (int interval);
#elif defined(__ANDROID__)
typedef EGLAPI    EGLBoolean        (EGLAPIENTRY* eglInitialize_fn)                             (EGLDisplay dpy, EGLint* major, EGLint* minor);
typedef EGLAPI    EGLDisplay        (EGLAPIENTRY* eglGetDisplay_fn)                             (EGLNativeDisplayType display_id);
typedef EGLAPI    EGLBoolean        (EGLAPIENTRY* eglChooseConfig_fn)                           (EGLDisplay dpy, const EGLint* attrib_list, EGLConfig* configs, EGLint config_size, EGLint* num_config);
typedef EGLAPI    EGLContext        (EGLAPIENTRY* eglCreateContext_fn)                          (EGLDisplay dpy, EGLConfig config, EGLContext share_context, const EGLint* attrib_list);
typedef EGLAPI    EGLBoolean        (EGLAPIENTRY* eglDestroyImageKHR_fn)                        (EGLDisplay dpy, EGLImageKHR image);
typedef EGLAPI    EGLImageKHR       (EGLAPIENTRY* eglCreateImageKHR_fn)                         (EGLDisplay dpy, EGLContext ctx, EGLenum target, EGLClientBuffer buffer, const EGLint* attrib_list);
typedef EGLAPI    EGLBoolean        (EGLAPIENTRY* eglSwapInterval_fn)                           (EGLDisplay dpy, EGLint interval);
typedef GL_API    void              (GL_APIENTRY* glEGLImageTargetTexture2DOES_fn)              (GLenum target, GLeglImageOES image);
typedef GL_API    void              (GL_APIENTRY* glReadPixels_fn)                              (GLint x, GLint y, GLsizei width, GLsizei height, GLenum format, GLenum type, GLvoid* pixels);
#elif defined(__linux)
typedef           void              (GLAPIENTRY* glXDestroyContext_fn)                          (Display* dpy, GLXContext ctx);
typedef           void              (GLAPIENTRY* glXDestroyWindow_fn)                           (Display* dpy, GLXWindow window);
typedef           void              (GLAPIENTRY* glXSwapBuffers_fn)                             (Display* dpy, GLXDrawable drawable);
typedef           Bool              (GLAPIENTRY* glXQueryExtension_fn)                          (Display* dpy, int* errorb, int* event);
typedef           GLXFBConfig*      (GLAPIENTRY* glXChooseFBConfig_fn)                          (Display* dpy, int screen, const int* attribList, int* nitems );
typedef           GLXWindow         (GLAPIENTRY* glXCreateWindow_fn)                            (Display* dpy, GLXFBConfig config, Window win, const int* attribList );
typedef           GLXContext        (GLAPIENTRY* glXCreateNewContext_fn)                        (Display* dpy, GLXFBConfig config, int renderType, GLXContext shareList, Bool direct );
typedef           Bool              (GLAPIENTRY* glXMakeCurrent_fn)                             (Display* dpy, GLXDrawable drawable, GLXContext ctx);
typedef           GLXContext        (GLAPIENTRY* glXGetCurrentContext_fn)                       (void);
typedef           GLXDrawable       (GLAPIENTRY* glXGetCurrentDrawable_fn)                      (void);
typedef           const char*       (GLAPIENTRY* glXQueryExtensionsString_fn)                   (Display* dpy, int screen);
typedef           void              (GLAPIENTRY* glXSwapIntervalEXT_fn)                         (Display* dpy, GLXDrawable drawable, int interval);
#endif

// Target
#define GL_DEPTH_BUFFER                                     0x8223
#define GL_STENCIL_BUFFER                                   0x8224
#define GL_ARRAY_BUFFER                                     0x8892
#define GL_ELEMENT_ARRAY_BUFFER                             0x8893
#define GL_PIXEL_PACK_BUFFER                                0x88EB
#define GL_PIXEL_UNPACK_BUFFER                              0x88EC
#define GL_UNIFORM_BUFFER                                   0x8A11
#define GL_TEXTURE_BUFFER                                   0x8C2A
#define GL_TRANSFORM_FEEDBACK_BUFFER                        0x8C8E
#define GL_READ_FRAMEBUFFER                                 0x8CA8
#define GL_DRAW_FRAMEBUFFER                                 0x8CA9
#define GL_FRAMEBUFFER                                      0x8D40
#define GL_RENDERBUFFER                                     0x8D41
#define GL_COPY_READ_BUFFER                                 0x8F36
#define GL_COPY_WRITE_BUFFER                                0x8F37
#define GL_DRAW_INDIRECT_BUFFER                             0x8F3F
#define GL_SHADER_STORAGE_BUFFER                            0x90D2
#define GL_DISPATCH_INDIRECT_BUFFER                         0x90EE
#define GL_QUERY_BUFFER                                     0x9192
#define GL_ATOMIC_COUNTER_BUFFER                            0x92C0
                                                        
// Attachments                                                                                   
#define GL_COLOR_ATTACHMENT0                                0x8CE0
#define GL_COLOR_ATTACHMENT_UNIT(x)  (GL_COLOR_ATTACHMENT0 + x)
#define GL_DEPTH_ATTACHMENT                                 0x8D00
#define GL_STENCIL_ATTACHMENT                               0x8D20

//  Frame Buffer Status
#define GL_FRAMEBUFFER_UNDEFINED                            0x8219
#define GL_FRAMEBUFFER_COMPLETE                             0x8CD5
#define GL_FRAMEBUFFER_INCOMPLETE_ATTACHMENT                0x8CD6
#define GL_FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT        0x8CD7
#define GL_FRAMEBUFFER_INCOMPLETE_DRAW_BUFFER               0x8CDB
#define GL_FRAMEBUFFER_INCOMPLETE_READ_BUFFER               0x8CDC
#define GL_FRAMEBUFFER_UNSUPPORTED                          0x8CDD
#define GL_FRAMEBUFFER_INCOMPLETE_MULTISAMPLE               0x8D56
#define GL_FRAMEBUFFER_INCOMPLETE_LAYER_TARGETS             0x8DA8

// Texture unit
#define GL_TEXTURE0 0x84C0
#define GL_TEXTURE_UNIT(x)    (GL_TEXTURE0 + x)

// Usage
#define GL_STREAM_DRAW                                      0x88E0
#define GL_STREAM_READ                                      0x88E1
#define GL_STREAM_COPY                                      0x88E2
#define GL_STATIC_DRAW                                      0x88E4
#define GL_STATIC_READ                                      0x88E5
#define GL_STATIC_COPY                                      0x88E6
#define GL_DYNAMIC_DRAW                                     0x88E8
#define GL_DYNAMIC_READ                                     0x88E9
#define GL_DYNAMIC_COPY                                     0x88EA

// Shader Type
#define GL_FRAGMENT_SHADER                                  0x8B30
#define GL_VERTEX_SHADER                                    0x8B31
#define GL_GEOMETRY_SHADER                                  0x8DD9
#define GL_TESS_EVALUATION_SHADER                           0x8E87
#define GL_TESS_CONTROL_SHADER                              0x8E88
#define GL_COMPUTE_SHADER                                   0x91B9

// Shader Info
#define GL_DELETE_STATUS                                    0x8B80
#define GL_COMPILE_STATUS                                   0x8B81
#define GL_LINK_STATUS                                      0x8B82
#define GL_VALIDATE_STATUS                                  0x8B83
#define GL_INFO_LOG_LENGTH                                  0x8B84

// Sampler params
#define GL_TEXTURE_MIN_LOD                                  0x813A
#define GL_TEXTURE_MAX_LOD                                  0x813B
#define GL_TEXTURE_WRAP_R                                   0x8072
#define GL_TEXTURE_COMPARE_MODE                             0x884C
#define GL_TEXTURE_COMPARE_FUNC                             0x884D

struct OpenGLImportTable
{
    OpenGLImportTable();
    ~OpenGLImportTable();

    AMF_RESULT  LoadFunctionsTable();
    AMF_RESULT  LoadContextFunctionsTable();

    amf_handle                                  m_hOpenGLDll;

    // Core
    glGetError_fn                               glGetError;
    glGetString_fn                              glGetString;
//    glGetStringi_fn                             glGetStringi;

    glEnable_fn                                 glEnable;
    glClear_fn                                  glClear;
    glClearAccum_fn                             glClearAccum;
    glClearColor_fn                             glClearColor;
    glClearDepth_fn                             glClearDepth;
    glClearIndex_fn                             glClearIndex;
    glClearStencil_fn                           glClearStencil;
    glDrawArrays_fn                             glDrawArrays;
    glViewport_fn                               glViewport;
    glFinish_fn                                 glFinish;

    // Core (platform-dependent)
#if defined(_WIN32)
    wglCreateContext_fn                         wglCreateContext;
    wglDeleteContext_fn                         wglDeleteContext;
    wglGetCurrentContext_fn                     wglGetCurrentContext;
    wglGetCurrentDC_fn                          wglGetCurrentDC;
    wglMakeCurrent_fn                           wglMakeCurrent;
    wglGetProcAddress_fn                        wglGetProcAddress;
    wglGetExtensionsStringARB_fn                wglGetExtensionsStringARB;
    wglSwapIntervalEXT_fn                       wglSwapIntervalEXT;
#elif defined(__ANDROID__)
    eglInitialize_fn                            eglInitialize;
    eglGetDisplay_fn                            eglGetDisplay;
    eglChooseConfig_fn                          eglChooseConfig;
    eglCreateContext_fn                         eglCreateContext;
    eglDestroyImageKHR_fn                       eglDestroyImageKHR;
    eglCreateImageKHR_fn                        eglCreateImageKHR;
    eglSwapInterval_fn                          eglSwapInterval;
    glEGLImageTargetTexture2DOES_fn             glEGLImageTargetTexture2DOES;
    glReadPixels_fn                             glReadPixels;
#elif defined(__linux)
    glXDestroyContext_fn                        glXDestroyContext;
    glXDestroyWindow_fn                         glXDestroyWindow;
    glXSwapBuffers_fn                           glXSwapBuffers;
    glXQueryExtension_fn                        glXQueryExtension;
    glXChooseFBConfig_fn                        glXChooseFBConfig;
    glXCreateWindow_fn                          glXCreateWindow;
    glXCreateNewContext_fn                      glXCreateNewContext;
    glXMakeCurrent_fn                           glXMakeCurrent;
    glXGetCurrentContext_fn                     glXGetCurrentContext;
    glXGetCurrentDrawable_fn                    glXGetCurrentDrawable;
    glXQueryExtensionsString_fn                 glXQueryExtensionsString;
    glXSwapIntervalEXT_fn                       glXSwapIntervalEXT;
#endif

    // Textures
    glBindTexture_fn                            glBindTexture;
    glDeleteTextures_fn                         glDeleteTextures;
    glGenTextures_fn                            glGenTextures;
    glGetTexImage_fn                            glGetTexImage;
    glGetTexLevelParameteriv_fn                 glGetTexLevelParameteriv;
    glTexParameteri_fn                          glTexParameteri;
    glTexImage2D_fn                             glTexImage2D;
    glActiveTexture_fn                          glActiveTexture;

    // Frame buffer and render buffer objects
    glBindFramebuffer_fn                        glBindFramebuffer;
//    glBindRenderbuffer_fn                       glBindRenderbuffer;
    glBlitFramebuffer_fn                        glBlitFramebuffer;
    glCheckFramebufferStatus_fn                 glCheckFramebufferStatus;
    glDeleteFramebuffers_fn                     glDeleteFramebuffers;
//    glDeleteRenderbuffers_fn                    glDeleteRenderbuffers;
//    glFramebufferRenderbuffer_fn                glFramebufferRenderbuffer;
//    glFramebufferTexture1D_fn                   glFramebufferTexture1D;
    glFramebufferTexture2D_fn                   glFramebufferTexture2D;
//    glFramebufferTexture3D_fn                   glFramebufferTexture3D;
    glFramebufferTextureLayer_fn                glFramebufferTextureLayer;
    glGenFramebuffers_fn                        glGenFramebuffers;
//    glGenRenderbuffers_fn                       glGenRenderbuffers;
//    glGenerateMipmap_fn                         glGenerateMipmap;
//    glGetFramebufferAttachmentParameteriv_fn    glGetFramebufferAttachmentParameteriv;
//    glGetRenderbufferParameteriv_fn             glGetRenderbufferParameteriv;
//    glIsFramebuffer_fn                          glIsFramebuffer;
//    glIsRenderbuffer_fn                         glIsRenderbuffer;
//    glRenderbufferStorage_fn                    glRenderbufferStorage;
//    glRenderbufferStorageMultisample_fn         glRenderbufferStorageMultisample;

    // Buffers
    glGenBuffers_fn                             glGenBuffers;
    glBindBuffer_fn                             glBindBuffer;
    glBufferData_fn                             glBufferData;
    glBufferSubData_fn                          glBufferSubData;
    glDeleteBuffers_fn                          glDeleteBuffers;

    // Vertex attributes
    glVertexAttribPointer_fn                    glVertexAttribPointer;
//    glVertexAttribLPointer_fn                   glVertexAttribLPointer;
//    glVertexAttribIPointer_fn                   glVertexAttribIPointer;
    glBindVertexBuffer_fn                       glBindVertexBuffer;
    glDisableVertexAttribArray_fn               glDisableVertexAttribArray;
    glEnableVertexAttribArray_fn                glEnableVertexAttribArray;

    // Vertex array objects
    glBindVertexArray_fn                        glBindVertexArray;
    glDeleteVertexArrays_fn                     glDeleteVertexArrays;
    glGenVertexArrays_fn                        glGenVertexArrays;
    glIsVertexArray_fn                          glIsVertexArray;

    // Shaders
    glCreateShader_fn                           glCreateShader;
    glShaderSource_fn                           glShaderSource;
    glCompileShader_fn                          glCompileShader;
    glGetShaderInfoLog_fn                       glGetShaderInfoLog;
    glGetShaderSource_fn                        glGetShaderSource;
    glGetShaderiv_fn                            glGetShaderiv;
    glCreateProgram_fn                          glCreateProgram;
    glAttachShader_fn                           glAttachShader;
    glLinkProgram_fn                            glLinkProgram;
    glGetProgramInfoLog_fn                      glGetProgramInfoLog;
    glGetProgramiv_fn                           glGetProgramiv;
    glValidateProgram_fn                        glValidateProgram;
    glUseProgram_fn                             glUseProgram;
    glDeleteShader_fn                           glDeleteShader;
    glDeleteProgram_fn                          glDeleteProgram;

    // Uniforms
    glGetUniformLocation_fn                     glGetUniformLocation;
//    glUniform1f_fn                              glUniform1f;
//    glUniform1fv_fn                             glUniform1fv;
    glUniform1i_fn                              glUniform1i;
//    glUniform1iv_fn                             glUniform1iv;
//    glUniform2f_fn                              glUniform2f;
//    glUniform2fv_fn                             glUniform2fv;
//    glUniform2i_fn                              glUniform2i;
//    glUniform2iv_fn                             glUniform2iv;
//    glUniform3f_fn                              glUniform3f;
//    glUniform3fv_fn                             glUniform3fv;
//    glUniform3i_fn                              glUniform3i;
//    glUniform3iv_fn                             glUniform3iv;
//    glUniform4f_fn                              glUniform4f;
    glUniform4fv_fn                             glUniform4fv;
//    glUniform4i_fn                              glUniform4i;
//    glUniform4iv_fn                             glUniform4iv;
//    glUniformMatrix2fv_fn                       glUniformMatrix2fv;
//    glUniformMatrix3fv_fn                       glUniformMatrix3fv;
//    glUniformMatrix4fv_fn                       glUniformMatrix4fv;

    // Uniform buffer objects
    glBindBufferBase_fn                         glBindBufferBase;
    glBindBufferRange_fn                        glBindBufferRange;
    glGetUniformBlockIndex_fn                   glGetUniformBlockIndex;
    glUniformBlockBinding_fn                    glUniformBlockBinding;

    // Sampler objects
    glBindSampler_fn                            glBindSampler;
    glDeleteSamplers_fn                         glDeleteSamplers;
    glGenSamplers_fn                            glGenSamplers;
//    glGetSamplerParameterIiv_fn                 glGetSamplerParameterIiv;
//    glGetSamplerParameterIuiv_fn                glGetSamplerParameterIuiv;
//    glGetSamplerParameterfv_fn                  glGetSamplerParameterfv;
//    glGetSamplerParameteriv_fn                  glGetSamplerParameteriv;
//    glIsSampler_fn                              glIsSampler;
//    glSamplerParameterIiv_fn                    glSamplerParameterIiv;
//    glSamplerParameterIuiv_fn                   glSamplerParameterIuiv;
    glSamplerParameterf_fn                      glSamplerParameterf;
    glSamplerParameterfv_fn                     glSamplerParameterfv;
    glSamplerParameteri_fn                      glSamplerParameteri;
//    glSamplerParameteriv_fn                     glSamplerParameteriv;

private:

#if defined(_WIN32)
        WNDCLASSEX  wndClass;
        HWND        hDummyWnd;
        HDC         hDummyDC;
        HGLRC       hDummyOGLContext;

        AMF_RESULT CreateDummy();
        AMF_RESULT DestroyDummy();
#endif
};