use embed_manifest::manifest::{
    ActiveCodePage, DpiAwareness, ExecutionLevel, HeapType, MaxVersionTested, Setting, SupportedOS,
};
use embed_manifest::{embed_manifest, empty_manifest};

fn main() {
    let manifest = empty_manifest()
        .name(env!("CARGO_PKG_NAME"))
        .version(
            env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
            env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
            env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
            0,
        )
        .supported_os(SupportedOS::Windows10..=SupportedOS::Windows10)
        .max_version_tested(MaxVersionTested::Windows11Version22H2)
        .requested_execution_level(ExecutionLevel::AsInvoker)
        .long_path_aware(Setting::Enabled)
        .dpi_awareness(DpiAwareness::PerMonitorV2Only)
        .active_code_page(ActiveCodePage::Utf8)
        .heap_type(HeapType::SegmentHeap);
    embed_manifest(manifest).expect("unable to embed manifest file");

    println!("cargo:rerun-if-changed=build.rs");
}
