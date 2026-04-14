#pragma once

// Expose POSIX and GNU extensions (strdup, strndup, etc.) on Linux/glibc.
// Without this, -std=c11 hides POSIX functions and the compiler assumes
// they return int, which truncates 64-bit pointers on aarch64.
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

// No dynamic plugin loading
// #undef ENABLE_LTDL

// No expat XML parser
// #undef HAVE_EXPAT

// No optional libraries
// #undef HAVE_LIBZ
// #undef HAVE_PANGOCAIRO

// Platform features available on macOS and Linux
#define HAVE_DRAND48 1
#define HAVE_SRAND48 1
#define HAVE_SETENV 1
#define HAVE_STRCASESTR 1
#define HAVE_SYS_MMAN_H 1
#define HAVE_SYS_SELECT_H 1
#define HAVE_UNISTD_H 1
#define HAVE_DL_ITERATE_PHDR 0
#define HAVE_INTPTR_T 1

#ifndef __APPLE__
#define HAVE_MEMRCHR 1
#endif

// Build configuration
#define PACKAGE_VERSION "14.1.5"
#define DEFAULT_DPI 96
#define GVPLUGIN_CONFIG_FILE ""
#define BROWSER ""

#ifdef __APPLE__
#define DARWIN 1
#endif
