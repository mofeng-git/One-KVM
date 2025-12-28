Name: @PROJECT_NAME@
Description: Intel(R) Media SDK Dispatcher
Version: @MFX_VERSION_MAJOR@.@MFX_VERSION_MINOR@

prefix=@CMAKE_INSTALL_PREFIX@
libdir=@CMAKE_INSTALL_FULL_LIBDIR@
includedir=@CMAKE_INSTALL_FULL_INCLUDEDIR@
Libs: -L${libdir} -ldispatch_shared -lstdc++ -ldl
Cflags: -I${includedir} -I${includedir}/mfx -DMFX_DISPATCHER_EXPOSED_PREFIX

