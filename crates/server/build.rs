use std::path::PathBuf;
use std::process;

fn main() {
    // CARGO_MANIFEST_DIR always points to the package root (crates/server/),
    // regardless of CWD. This is the robust way to locate the web directory.
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let web_dir = manifest_dir.join("../../web");
    let dist_dir = web_dir.join("dist");

    // Tell Cargo to rerun this script if any web source files change
    println!("cargo:rerun-if-changed=../../web/package.json");
    println!("cargo:rerun-if-changed=../../web/package-lock.json");
    println!("cargo:rerun-if-changed=../../web/vite.config.ts");
    println!("cargo:rerun-if-changed=../../web/index.html");
    println!("cargo:rerun-if-changed=../../web/src");

    println!("cargo:rerun-if-env-changed=CODEPRISM_SKIP_WEB_BUILD");

    // CI and release jobs may build the frontend explicitly before Cargo. In
    // that case, reuse the verified dist directory and avoid doing the work twice.
    let skip_web_build = std::env::var_os("CODEPRISM_SKIP_WEB_BUILD").is_some();

    let npm_cmd = if cfg!(windows) { "npm.cmd" } else { "npm" };

    // Check if npm is available
    let has_npm = process::Command::new(npm_cmd)
        .arg("--version")
        .output()
        .is_ok();

    if has_npm && !skip_web_build {
        println!("cargo:warning=Building frontend assets...");

        // Dependency installation is intentionally kept outside Cargo builds.
        // This avoids network access and package-lock.json rewrites during Rust
        // checks while still keeping embedded assets in sync with source files.
        let build_status = process::Command::new(npm_cmd)
            .args(["run", "build"])
            .current_dir(&web_dir)
            .status();

        if build_status.map(|s| !s.success()).unwrap_or(true) {
            println!("cargo:warning=Frontend build failed. Will try to use existing assets.");
        }
    } else if !has_npm && !skip_web_build {
        println!("cargo:warning=npm not found. Skipping frontend build.");
    }

    if !dist_dir.exists() {
        eprintln!(
            "\n\nError: Frontend assets not found at '{}'\n",
            dist_dir.display()
        );
        eprintln!("The server requires the web frontend to be built.");
        if !has_npm {
            eprintln!("'npm' command was not found in your PATH.");
        } else {
            eprintln!("Automatic build with npm failed.");
        }
        eprintln!("Please manually build the frontend:");
        eprintln!("  cd web");
        eprintln!("  npm ci");
        eprintln!("  npm run build");
        eprintln!("\nThen try running cargo build again.\n\n");

        process::exit(1);
    }
}
