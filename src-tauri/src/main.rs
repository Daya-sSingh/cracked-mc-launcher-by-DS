// Thin by design: all real setup lives in `launcher_lib::run`, in lib.rs.
// Splitting it this way is what lets the same app logic also compile as a
// library (`crate-type = ["staticlib", "cdylib", "rlib"]` in Cargo.toml),
// which Tauri's mobile tooling and our own integration tests both rely on.
fn main() {
    launcher_lib::run();
}
