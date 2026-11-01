//! Probe the platform layer against a running compositor and print what it actually reports.
//!
//! Written for the Hyprland/Omarchy port, but it is deliberately compositor-agnostic: run it on KDE,
//! GNOME, XFCE or inside a nested Hyprland and it prints what the layer sees. The value is the same
//! either way — it is the difference between "the code compiles" and "the code answers".
//!
//! Run against a nested instance:
//!   XDG_RUNTIME_DIR=/run/user/1000 \
//!   HYPRLAND_INSTANCE_SIGNATURE=$(ls -t /run/user/1000/hypr/ | head -1) \
//!   cargo run --release --example platform_probe

pub fn main() {
    println!("=== environment ===");
    for var in [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_TYPE",
        "WAYLAND_DISPLAY",
        "DISPLAY",
        "HYPRLAND_INSTANCE_SIGNATURE",
    ] {
        println!("  {var:<28} {}", std::env::var(var).unwrap_or_else(|_| "(unset)".into()));
    }

    println!("\n=== capabilities ===");
    let caps = quixi_platform::linux::capabilities();
    println!("  session_type         {}", caps.session_type);
    println!("  screenshot           {} ({})", caps.screenshot, caps.screenshot_backend);
    println!("  screenshot_region    {}", caps.screenshot_region);
    println!("  clipboard            {}", caps.clipboard);
    println!("  input_synthesis      {}", caps.input_synthesis);
    println!("  file_pickers         {}", caps.file_pickers);
    println!("  display_listing      {}", caps.display_listing);
    println!("  app_launch           {}", caps.app_launch);
    println!("  window_layout        {}", caps.window_layout);
    for n in &caps.notes {
        println!("  note: {n}");
    }

    println!("\n=== cursor_position ===");
    match quixi_platform::linux::cursor_position() {
        Some((x, y)) => println!("  Some({x}, {y})"),
        None => println!("  None  (no compositor that publishes it, or no input backend)"),
    }

    println!("\n=== displays() ===");
    let ds = quixi_platform::linux::displays();
    if ds.is_empty() {
        println!("  (none)");
    }
    for d in &ds {
        println!("  id={:<3} {:<14} {}x{} @ {}x", d.id, d.name, d.width, d.height, d.scale);
    }

    println!("\n=== list_displays_json() ===");
    match quixi_platform::linux::list_displays_json() {
        Ok(v) => println!("  {}", serde_json::to_string(&v).unwrap_or_else(|_| "?".into())),
        Err(e) => println!("  Err: {e:#}"),
    }

    println!("\n=== keybindings ===");
    let kb = quixi_platform::linux::list_keybindings();
    println!("  total: {}", kb.len());
    let mut by_source: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for k in &kb {
        *by_source.entry(k.source.as_str()).or_default() += 1;
    }
    for (s, n) in by_source {
        println!("  {s}: {n}");
    }
    for k in kb.iter().filter(|k| !k.keys.is_empty()).take(12) {
        println!("    {:<28} {:<30} {}", k.keys, k.description, k.action);
    }
    if let Some(t) = quixi_platform::linux::find_keybinding("terminal") {
        println!("  lookup \"terminal\" -> {} = {}", t.keys, t.action);
    }
}
