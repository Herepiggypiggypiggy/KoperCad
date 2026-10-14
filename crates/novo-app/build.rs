//! Embeds the application icon, version info and a no-elevation manifest
//! into the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/novoelectric.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/novoelectric.ico");
        // Never ask for elevation: NovoElectric must run without admin rights.
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
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
