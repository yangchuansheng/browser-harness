use bh_wasm_host::{default_manifest, default_runner_config, operation_names};

fn print_usage() {
    eprintln!(
        "usage: bhrun <manifest|sample-config|capabilities|summary>\n\
         scaffold only: this binary does not execute WASM guests yet"
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("manifest") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&default_manifest()).expect("serialize manifest")
            );
        }
        Some("sample-config") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&default_runner_config())
                    .expect("serialize runner config")
            );
        }
        Some("capabilities") => {
            for name in operation_names() {
                println!("{name}");
            }
        }
        Some("summary") => {
            let manifest = default_manifest();
            println!(
                "bhrun scaffold: execution_model={:?} guest_transport={:?} protocol_families={} operations={}",
                manifest.execution_model,
                manifest.guest_transport,
                manifest.protocol_families.len(),
                manifest.operations.len()
            );
        }
        _ => print_usage(),
    }
}
