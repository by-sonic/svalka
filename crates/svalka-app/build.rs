fn main() {
    tauri_build::build();

    // Tauri embeds its own Windows manifest with `/MANIFESTUAC:NO`. Append a
    // UAC fragment afterwards (last `/MANIFESTUAC` wins) so Svalka launches
    // elevated — required to clean system folders such as C:\Windows\Temp and
    // SoftwareDistribution\Download.
    #[cfg(windows)]
    println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:level='requireAdministrator' uiAccess='false'");
}
