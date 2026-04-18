use std::io::Write;
use std::path::PathBuf;

fn main() {
    let crate_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let vendor = crate_dir.join("vendor/graphviz-14.1.5");
    let lib = vendor.join("lib");
    let generated = vendor.join("generated");

    // Graphviz headers use flat includes ("cdt.h", "cgraph.h", etc.),
    // so every library subdirectory must be on the include path.
    let lib_subdirs: Vec<&str> = vec![
        "cdt", "cgraph", "common", "dotgen", "gvc", "label", "ortho", "pack", "pathplan", "xdot",
        "util",
    ];

    let base = |_name: &str| -> cc::Build {
        let mut build = cc::Build::new();
        build
            .flag("-std=c11")
            .flag("-w") // suppress warnings for vendored code
            .flag("-include")
            .flag("config.h");

        if cfg!(target_os = "macos") {
            build.flag("-fno-common");
        }

        // Graphviz uses __attribute__((visibility("hidden"))) to hide
        // internal symbols.  This is correct for shared libraries but
        // breaks cross-archive symbol resolution on Linux when building
        // as static libraries.  Override to make all symbols visible.
        build.flag("-fvisibility=default");
        build.define("PRIVATE", None);
        build.define("UTIL_API", None);

        build.include(&crate_dir); // for config.h
        build.include(&generated); // for pre-generated headers
        build.include(&vendor); // for plugin includes
        build.include(&lib); // for <cdt/dthdr.h> style includes

        for subdir in &lib_subdirs {
            build.include(lib.join(subdir));
        }

        build
    };

    let collect_c_files = |dir: &str| -> Vec<PathBuf> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(lib.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "c") {
                files.push(path);
            }
        }
        files.sort();
        files
    };

    // --- cdt ---
    let mut cdt = base("cdt");
    for f in collect_c_files("cdt") {
        cdt.file(&f);
    }
    cdt.compile("cdt");

    // --- cgraph (uses pre-generated grammar.c, scan.c) ---
    let mut cgraph = base("cgraph");
    for f in collect_c_files("cgraph") {
        let name = f.file_name().unwrap().to_str().unwrap();
        if name == "grammar.c" || name == "scan.c" {
            continue; // use pre-generated versions
        }
        cgraph.file(&f);
    }
    cgraph.file(generated.join("grammar.c"));
    cgraph.file(generated.join("scan.c"));
    cgraph.compile("cgraph");

    // --- pathplan ---
    let mut pathplan = base("pathplan");
    for f in collect_c_files("pathplan") {
        pathplan.file(&f);
    }
    pathplan.compile("pathplan");

    // --- xdot ---
    let mut xdot = base("xdot");
    xdot.file(lib.join("xdot/xdot.c"));
    xdot.compile("xdot");

    // --- label ---
    let mut label = base("label");
    for f in collect_c_files("label") {
        label.file(&f);
    }
    label.compile("label");

    // --- pack ---
    let mut pack = base("pack");
    for f in collect_c_files("pack") {
        pack.file(&f);
    }
    pack.compile("pack");

    // --- ortho ---
    let mut ortho = base("ortho");
    for f in collect_c_files("ortho") {
        ortho.file(&f);
    }
    ortho.compile("ortho");

    // --- util ---
    let mut util = base("util");
    for f in collect_c_files("util") {
        util.file(&f);
    }
    util.compile("gvutil");

    // --- common (uses pre-generated htmlparse.c) ---
    let mut common = base("common");
    for f in collect_c_files("common") {
        let name = f.file_name().unwrap().to_str().unwrap();
        if name == "htmlparse.c" {
            continue; // use pre-generated version
        }
        common.file(&f);
    }
    common.file(generated.join("htmlparse.c"));
    common.compile("common");

    // --- dotgen ---
    let mut dotgen = base("dotgen");
    for f in collect_c_files("dotgen") {
        dotgen.file(&f);
    }
    dotgen.compile("dotgen");

    // --- gvc ---
    let mut gvc = base("gvc");
    for f in collect_c_files("gvc") {
        gvc.file(&f);
    }
    gvc.compile("gvc");

    // --- plugin/core (all renderers since gvplugin_core.c references them all) ---
    let mut core_plugin = base("common");
    core_plugin.include(vendor.join("plugin/core"));
    for entry in std::fs::read_dir(vendor.join("plugin/core")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "c") {
            core_plugin.file(&path);
        }
    }
    core_plugin.compile("gvplugin_core");

    // --- plugin/dot_layout ---
    let mut dot_layout_plugin = base("dotgen");
    dot_layout_plugin.file(vendor.join("plugin/dot_layout/gvlayout_dot_layout.c"));
    dot_layout_plugin.file(vendor.join("plugin/dot_layout/gvplugin_dot_layout.c"));
    dot_layout_plugin.compile("gvplugin_dot_layout");

    // --- builtins (static plugin registration) ---
    let mut builtins = base("gvc");
    builtins.file(crate_dir.join("builtins.c"));
    builtins.compile("builtins");

    // Link order matters -- libraries must come after their dependents.
    // On Linux, the single-pass static linker cannot resolve circular deps
    // between static archives.  common/emit.c references gvevent symbols from
    // gvc, but gvc depends on common.  We merge all archives into one combined
    // archive using GNU ar's MRI script mode, which eliminates the cycle.
    // On macOS the linker handles cycles natively, so we link each lib directly.
    let all_libs = [
        "builtins",
        "gvplugin_dot_layout",
        "gvplugin_core",
        "gvc",
        "dotgen",
        "common",
        "gvutil",
        "ortho",
        "pack",
        "label",
        "xdot",
        "pathplan",
        "cgraph",
        "cdt",
    ];

    if cfg!(target_os = "linux") {
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let combined = out_dir.join("libgraphviz_combined.a");
        let mut ar_script = String::from("CREATE ");
        ar_script.push_str(combined.to_str().unwrap());
        ar_script.push('\n');
        for lib_name in &all_libs {
            let lib_path = out_dir.join(format!("lib{lib_name}.a"));
            ar_script.push_str("ADDLIB ");
            ar_script.push_str(lib_path.to_str().unwrap());
            ar_script.push('\n');
        }
        ar_script.push_str("SAVE\nEND\n");

        let ar_cmd = std::env::var("AR").unwrap_or_else(|_| "ar".to_string());
        #[allow(clippy::disallowed_methods)]
        let mut child = std::process::Command::new(&ar_cmd)
            .arg("-M")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to run {ar_cmd}: {e}"));
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(ar_script.as_bytes())
            .unwrap();
        let status = child.wait().unwrap();
        assert!(status.success(), "ar -M failed");

        println!("cargo:rustc-link-search=native={}", out_dir.display());
        println!("cargo:rustc-link-lib=static=graphviz_combined");
    } else {
        for lib_name in &all_libs {
            println!("cargo:rustc-link-lib=static={lib_name}");
        }
    }
}
