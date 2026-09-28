//! Gives the Windows program its icon and version information (the file's
//! Details tab: product, description, version, license and original file
//! name). Other targets need nothing: the macOS app carries its icon in
//! the bundle (`tools/package/macos.sh`) and every window gets it at run
//! time.

use std::io;

const ICON: &str = "../../assets/icons/re-zoids-saga.ico";
const PRODUCT: &str = "Re:Zoids Saga";

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed={ICON}");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }
    let license = std::env::var("CARGO_PKG_LICENSE").unwrap_or_default();
    winresource::WindowsResource::new()
        .set_icon(ICON)
        .set("ProductName", PRODUCT)
        .set("FileDescription", PRODUCT)
        .set("CompanyName", "The Re:Zoids Saga project")
        .set("LegalCopyright", &license)
        .set("InternalName", "re-zoids-saga")
        .set("OriginalFilename", "re-zoids-saga.exe")
        .compile()
}
