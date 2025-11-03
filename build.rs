use std::path::Path;
use std::process::Command;

fn main() {
    let webui_dir = Path::new("webui");
    let webui_out = webui_dir.join("out");
    println!("cargo:rerun-if-changed=webui/package.json");
    println!("cargo:rerun-if-changed=webui/src");
    println!("cargo:rerun-if-changed=webui/next.config.ts");

    if !webui_out.exists() {
        println!("cargo:info=WebUI not built. Building Next.js static export...");
        if !webui_dir.join("node_modules").exists() {
            println!("cargo:info=Installing bun dependencies for webui...");
            let install_status = if cfg!(target_os = "windows") {
                Command::new("cmd").args(&["/C", "cd webui && bun install"]).status()
            } else {
                Command::new("sh").arg("-c").arg("cd webui && bun install").status()
            };

            if !install_status.map(|s| s.success()).unwrap_or(false) {
                println!("cargo:warning=Failed to install webui dependencies");
            }
        }

        let build_status = if cfg!(target_os = "windows") {
            Command::new("cmd").args(&["/C", "cd webui && bun run build"]).status()
        } else {
            Command::new("sh").arg("-c").arg("cd webui && bun run build").status()
        };

        if !build_status.map(|s| s.success()).unwrap_or(false) {
            println!("cargo:warning=Failed to build webui - static files may not be available");
        } else {
            println!("cargo:info=WebUI built successfully!");
        }
    } else {
        println!("cargo:info=WebUI static files found at webui/out");
    }
}
