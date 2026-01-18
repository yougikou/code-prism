use std::path::Path;
use std::process;

fn main() {
    println!("cargo:rerun-if-changed=../../web/dist");

    let dist_path = Path::new("../../web/dist");
    if !dist_path.exists() {
        eprintln!(
            "\n\nError: Frontend assets not found at '{}'\n",
            dist_path.display()
        );
        eprintln!("The server requires the web frontend to be built first.");
        eprintln!("Please run the following commands in the 'web' directory:");
        eprintln!("  cd web");
        eprintln!("  npm install");
        eprintln!("  npm run build");
        eprintln!("\nThen try running 'cargo run -- init' again.\n\n");

        // Exit with a non-zero status to fail the build
        process::exit(1);
    }
}
