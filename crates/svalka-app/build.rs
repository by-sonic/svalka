fn main() {
    // Svalka must run elevated: it cleans system folders (C:\Windows\Temp,
    // SoftwareDistribution\Download) and moves junk into a per-volume store at
    // the drive root (`<volume>\.svalka-quarantine`), all of which require
    // administrator rights.
    //
    // The earlier `/MANIFESTUAC` linker arg silently did nothing — tauri-build
    // embeds its own application manifest (RT_MANIFEST), and that embedded
    // resource wins over a linker-generated one. So we instead hand tauri-build
    // a complete manifest (Common-Controls + requireAdministrator) to embed.
    #[cfg(windows)]
    {
        let manifest = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#;
        let attrs = tauri_build::Attributes::new().windows_attributes(
            tauri_build::WindowsAttributes::new().app_manifest(manifest),
        );
        tauri_build::try_build(attrs).expect("failed to run tauri-build");
    }

    #[cfg(not(windows))]
    tauri_build::build();
}
