fn main() {
    for name in ["PROFILE", "OPT_LEVEL"] {
        if let Ok(value) = std::env::var(name) {
            println!("cargo:rustc-env=AGENTKIT_BUILD_{name}={value}");
        }
    }
}
