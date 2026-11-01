//! Linux-only probe of the platform layer; the implementation lives in `linux/platform_probe.rs`.
//! On other operating systems this example only explains that it does not apply.

#[cfg(target_os = "linux")]
#[path = "linux/platform_probe.rs"]
mod imp;

fn main() {
    #[cfg(target_os = "linux")]
    imp::main();
    #[cfg(not(target_os = "linux"))]
    eprintln!("platform_probe exercises the Linux platform layer and only runs on Linux.");
}
