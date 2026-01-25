use std::path::Path;
use std::process;

fn main() {
    let web_dir = Path::new("../../web");
    let dist_dir = web_dir.join("dist");

    // Tell Cargo to rerun this script if any web source files change
    println!("cargo:rerun-if-changed=../../web/package.json");
    println!("cargo:rerun-if-changed=../../web/vite.config.ts");
    println!("cargo:rerun-if-changed=../../web/index.html");
    // We can't list every file in src easily without walking, but we can list the directory
    // Note: Cargo rerun-if-changed on a directory only detects if the directory entry itself changes (file added/removed),
    // not if content of files inside changes, usually.
    // Ideally we'd walk the tree, but for now let's just trigger on key files.
    // If the user is editing web code, they likely want a rebuild.
    // A better approach for robust dev is to use the separate dev server.
    // This build script is mainly for "cargo build --release" or initial setup.
    println!("cargo:rerun-if-changed=../../web/src");

    // Only attempt to build if we are in a build where we actually need the assets?
    // Doing it always ensures consistency.

    let npm_cmd = if cfg!(windows) { "npm.cmd" } else { "npm" };

    // Check if npm is available
    let has_npm = process::Command::new(npm_cmd)
        .arg("--version")
        .output()
        .is_ok();

    if has_npm {
        println!("cargo:warning=Building frontend assets...");

        // npm install
        let install_status = process::Command::new(npm_cmd)
            .arg("install")
            .current_dir(web_dir)
            .status();

        match install_status {
            Ok(status) if status.success() => {
                // npm run build
                let build_status = process::Command::new(npm_cmd)
                    .args(["run", "build"])
                    .current_dir(web_dir)
                    .status();

                if build_status.map(|s| !s.success()).unwrap_or(true) {
                    println!(
                        "cargo:warning=Frontend build failed. Will try to use existing assets."
                    );
                }
            }
            _ => {
                println!("cargo:warning=npm install failed. Will try to use existing assets.");
            }
        }
    } else {
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
        eprintln!("  npm install");
        eprintln!("  npm run build");
        eprintln!("\nThen try running cargo build again.\n\n");

        process::exit(1);
    }
}
