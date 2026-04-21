use bh_wasm_host::default_capabilities;

fn main() {
    let capabilities = default_capabilities();
    println!(
        "bhrun scaffold: {} host capabilities defined",
        capabilities.len()
    );
}
