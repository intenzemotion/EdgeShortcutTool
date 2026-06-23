#[cfg(windows)]
fn main() {
    use std::path::Path;

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=edge.ico");

    let mut res = winresource::WindowsResource::new();

    res.set("ProductName", "Edge Shortcut Tool");
    res.set("FileDescription", "Edge Shortcut Tool");
    res.set("ProductVersion", "1.4.0.0");
    res.set("FileVersion", "1.4.0.0");

    if Path::new("edge.ico").exists() {
        res.set_icon("edge.ico");
    }

    if Path::new("app.manifest").exists() {
        res.set_manifest_file("app.manifest");
    }

    res.compile().expect("failed to compile Windows resources");
}

#[cfg(not(windows))]
fn main() {}
