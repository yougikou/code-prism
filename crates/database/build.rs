fn main() {
    // Re-run the build script if the init schema changes.
    println!("cargo:rerun-if-changed=src/init.sql");
}
