fn main() {
    // Tell Cargo to re-run the build script if any migration file changes.
    // This ensures new or modified SQL migrations trigger a recompilation
    // of the database crate (which embeds migrations via sqlx::migrate!).
    println!("cargo:rerun-if-changed=../../migrations");
}
