#![allow(unsafe_code)]

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uint};
use std::ptr;
use std::sync::Mutex;

// Opaque type for lt_symlist_t from gvcext.h
#[repr(C)]
struct LtSymlist {
    name:    *const c_char,
    address: *mut std::ffi::c_void,
}

extern "C" {
    // The builtins.c array that registers our statically-linked plugins.
    // Declared as a single element (not zero-sized) because Rust ZSTs have no
    // guaranteed ABI layout; we only ever pass the pointer, never index.
    static lt_preloaded_symbols: LtSymlist;
}

// Manual FFI declarations (no bindgen needed -- we only use 8 functions)
extern "C" {
    fn gvContextPlugins(builtins: *const LtSymlist, demand_loading: c_int)
    -> *mut std::ffi::c_void;
    fn agmemread(cp: *const c_char) -> *mut std::ffi::c_void;
    fn gvLayout(
        gvc: *mut std::ffi::c_void,
        g: *mut std::ffi::c_void,
        engine: *const c_char,
    ) -> c_int;
    fn gvRenderData(
        gvc: *mut std::ffi::c_void,
        g: *mut std::ffi::c_void,
        format: *const c_char,
        result: *mut *mut c_char,
        length: *mut c_uint,
    ) -> c_int;
    fn gvFreeRenderData(data: *mut c_char);
    fn gvFreeLayout(gvc: *mut std::ffi::c_void, g: *mut std::ffi::c_void) -> c_int;
    fn agclose(g: *mut std::ffi::c_void) -> c_int;
    fn gvFreeContext(gvc: *mut std::ffi::c_void) -> c_int;
}

/// Global mutex to serialize Graphviz FFI calls.
///
/// Graphviz has internal global state (string interning, error counters, layout
/// algorithm state) that is not thread-safe.  Since `render_dot_to_svg` is
/// already called via `spawn_blocking` in the server, this mutex adds
/// negligible overhead while preventing data races.
static RENDER_LOCK: Mutex<()> = Mutex::new(());

/// Render a DOT source string to SVG bytes using the vendored Graphviz library.
///
/// Serialized via a global mutex because the Graphviz C library has internal
/// global state that is not thread-safe.
/// Returns the SVG output as a byte vector, or an error message.
pub fn render_dot_to_svg(dot_source: &str) -> Result<Vec<u8>, String> {
    let source = CString::new(dot_source).map_err(|e| format!("invalid DOT source: {e}"))?;
    let engine = CString::new("dot").unwrap();
    let format = CString::new("svg").unwrap();

    let _guard = RENDER_LOCK
        .lock()
        .map_err(|e| format!("render lock poisoned: {e}"))?;

    // SAFETY: All pointers are checked for null. Resources are freed in reverse
    // order. The global mutex serializes access to Graphviz's non-thread-safe
    // internal state.
    unsafe {
        let gvc = gvContextPlugins(&raw const lt_preloaded_symbols, 0);
        if gvc.is_null() {
            return Err("failed to create Graphviz context".into());
        }

        let graph = agmemread(source.as_ptr());
        if graph.is_null() {
            gvFreeContext(gvc);
            return Err("failed to parse DOT source".into());
        }

        let rc = gvLayout(gvc, graph, engine.as_ptr());
        if rc != 0 {
            agclose(graph);
            gvFreeContext(gvc);
            return Err("Graphviz layout failed".into());
        }

        let mut buf: *mut c_char = ptr::null_mut();
        let mut len: c_uint = 0;
        let rc = gvRenderData(gvc, graph, format.as_ptr(), &raw mut buf, &raw mut len);
        if rc != 0 || buf.is_null() {
            gvFreeLayout(gvc, graph);
            agclose(graph);
            gvFreeContext(gvc);
            return Err("Graphviz render failed".into());
        }

        let result = std::slice::from_raw_parts(buf as *const u8, len as usize).to_vec();

        gvFreeRenderData(buf);
        gvFreeLayout(gvc, graph);
        agclose(graph);
        gvFreeContext(gvc);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_dot_to_svg() {
        let svg = render_dot_to_svg("digraph { a -> b }").unwrap();
        let svg_str = String::from_utf8(svg).unwrap();
        assert!(svg_str.contains("<svg"));
        assert!(svg_str.contains("</svg>"));
    }

    #[test]
    fn malformed_dot_returns_error() {
        let result = render_dot_to_svg("not valid dot");
        assert!(result.is_err());
    }

    #[test]
    fn concurrent_renders_are_safe() {
        let handles: Vec<_> = (0..8)
            .map(|i| {
                #[expect(clippy::disallowed_methods, reason = "Intentional OS threads to verify Mutex-based thread safety of vendored Graphviz FFI")]
                std::thread::spawn(move || {
                    let source = format!("digraph {{ node{i} -> node{} }}", i + 1);
                    render_dot_to_svg(&source).unwrap()
                })
            })
            .collect();

        for handle in handles {
            let svg = handle.join().unwrap();
            assert!(!svg.is_empty());
        }
    }
}
