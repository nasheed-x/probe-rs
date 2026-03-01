use std::io::Cursor;

use probe_rs::config::TargetSelector;
use probe_rs::flashing::{DownloadOptions, Format};
use probe_rs::probe::list::Lister;
use probe_rs::Permissions;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct ProbeInfo {
    identifier: String,
    vendor_id: u16,
    product_id: u16,
    serial_number: Option<String>,
    probe_type: String,
}

/// Lists all connected debug probes. Returns JSON array.
#[wasm_bindgen]
pub async fn list_probes() -> Result<String, JsValue> {
    let lister = Lister::new();
    let probes = lister.list_all().await;

    let infos: Vec<ProbeInfo> = probes
        .iter()
        .map(|p| ProbeInfo {
            identifier: p.identifier.clone(),
            vendor_id: p.vendor_id,
            product_id: p.product_id,
            serial_number: p.serial_number.clone(),
            probe_type: format!("{:?}", p.probe_type()),
        })
        .collect();

    serde_json::to_string(&infos).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Connect to the first available probe, auto-detect the chip, and return its name.
#[wasm_bindgen]
pub async fn detect_target() -> Result<String, JsValue> {
    let lister = Lister::new();
    let probes = lister.list_all().await;
    if probes.is_empty() {
        return Err(JsValue::from_str("No debug probe found."));
    }

    let probe = lister
        .open(probes[0].clone())
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to open probe: {e}")))?;

    let session = probe
        .attach(TargetSelector::Auto, Permissions::default())
        .await
        .map_err(|e| JsValue::from_str(&format!("Auto-detect failed: {e}")))?;

    Ok(session.target().name.clone())
}

/// Flash firmware to the auto-detected target.
///
/// - `firmware_data`: raw bytes of the firmware file
/// - `format`: "elf", "hex", or "bin"
/// - `progress_callback`: JS function(phase: string, pct: number)
///
/// Returns the detected chip name on success.
#[wasm_bindgen]
pub async fn flash_firmware(
    firmware_data: &[u8],
    format: &str,
    progress_callback: &js_sys::Function,
) -> Result<String, JsValue> {
    let report = |phase: &str, pct: f64| {
        let _ = progress_callback.call2(
            &JsValue::NULL,
            &JsValue::from_str(phase),
            &JsValue::from_f64(pct),
        );
    };

    report("Connecting to probe...", 0.0);

    let lister = Lister::new();
    let probes = lister.list_all().await;
    if probes.is_empty() {
        return Err(JsValue::from_str("No debug probe found."));
    }

    report("Opening probe...", 10.0);

    let probe = lister
        .open(probes[0].clone())
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to open probe: {e}")))?;

    report("Auto-detecting target...", 20.0);

    let mut session = probe
        .attach(TargetSelector::Auto, Permissions::default())
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to attach: {e}")))?;

    let chip_name = session.target().name.clone();
    report(&format!("Detected: {chip_name}"), 30.0);

    report("Loading firmware image...", 40.0);

    let mut loader = session.target().flash_loader();
    let fmt: Format = match format {
        "hex" => Format::Hex,
        "bin" => Format::Bin(Default::default()),
        _ => Format::Elf(Default::default()),
    };

    let mut cursor = Cursor::new(firmware_data.to_vec());
    loader
        .load_image(&mut session, &mut cursor, fmt, None)
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to load image: {e}")))?;

    report("Erasing and programming flash...", 60.0);

    loader
        .commit(&mut session, DownloadOptions::default())
        .await
        .map_err(|e| JsValue::from_str(&format!("Flash failed: {e}")))?;

    report("Done!", 100.0);

    Ok(chip_name)
}
