fn main() {
    println!("cargo:rerun-if-changed=../target/wasm32-wasip1/release/zellij_notify.wasm");
}
