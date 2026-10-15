//! Embeds the application icon, version info and a no-elevation manifest
//! into the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/branding/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/branding/icon.ico");
    // Never ask for elevation: KoperCAD must run without admin rights.
    res.set_manifest(
        r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
  <security>
    <requestedPrivileges>
      <requestedExecutionLevel level="asInvoker" uiAccess="false" />
    </requestedPrivileges>
  </security>
</trustInfo>
</assembly>"#,
    );
    // Fail the build rather than ship an exe without the asInvoker manifest.
    res.compile()
        .expect("could not embed the Windows icon and manifest");
}
