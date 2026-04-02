#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    let primary = std::path::Path::new("../icon/ReaderIcon2.ico");
    let fallback = std::path::Path::new("../icon/ReaderIcon.ico");
    if primary.exists() {
        res.set_icon(primary.to_string_lossy().as_ref());
    } else {
        res.set_icon(fallback.to_string_lossy().as_ref());
    }
    res.compile()
        .expect("failed to compile Windows icon resources");
}

#[cfg(not(windows))]
fn main() {}
