/// Download raw CFDIs from SAT to cfdi_data/ for local inspection.
/// Usage: cargo run --bin inspect_cfdis -- <start YYYY-MM-DD> <end YYYY-MM-DD>
/// Example: cargo run --bin inspect_cfdis -- 2022-01-01 2026-04-21

use std::path::PathBuf;

use alfredodev::sat::{CfdiDownloadRequest, DownloadType, RequestType, download_cfdis};
use dotenvy::dotenv;
use zip::ZipArchive;

const RFC: &str = "RIAL8907172J3";
const CER_PATH: &str = "tmp/FIEL_RIAL8907172J3_20221215134700/rial8907172j3.cer";
const KEY_PATH: &str = "tmp/FIEL_RIAL8907172J3_20221215134700/Claveprivada_FIEL_RIAL8907172J3_20221215_134700.key";
const KEY_PASSWORD: &str = "AxelNicole1303";

#[tokio::main]
async fn main() {
    dotenv().ok();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <start YYYY-MM-DD> <end YYYY-MM-DD>", args[0]);
        std::process::exit(1);
    }

    let start = format!("{}T00:00:00", args[1]);
    let end = format!("{}T23:59:59", args[2]);

    let out_root = PathBuf::from("cfdi_data");
    tokio::fs::create_dir_all(&out_root).await.unwrap();

    for dl_type in [DownloadType::Issued, DownloadType::Received] {
        let label = dl_type.env_value();
        println!("--- Descargando {label} ({start} → {end}) ---");

        let out_dir = out_root.join(label);
        tokio::fs::create_dir_all(&out_dir).await.unwrap();

        let request = CfdiDownloadRequest {
            cer_path: Some(CER_PATH.to_string()),
            key_path: Some(KEY_PATH.to_string()),
            key_password: Some(KEY_PASSWORD.to_string()),
            rfc: Some(RFC.to_string()),
            download_type: dl_type,
            request_type: RequestType::Xml,
            start: Some(start.clone()),
            end: Some(end.clone()),
            output_dir: Some(out_dir.to_string_lossy().to_string()),
            poll_seconds: None,
            max_attempts: None,
        };

        match download_cfdis("local", request).await {
            Err(e) => eprintln!("Error descargando {label}: {e}"),
            Ok(result) => {
                println!("  {} paquete(s) descargado(s)", result.packages.len());
                for pkg in &result.packages {
                    println!("  Extrayendo {}...", pkg.package_id);
                    let xml_dir = out_dir.join(&pkg.package_id);
                    std::fs::create_dir_all(&xml_dir).unwrap();

                    let zip_bytes = std::fs::read(&pkg.path).unwrap();
                    let mut archive = ZipArchive::new(std::io::Cursor::new(zip_bytes)).unwrap();
                    let mut count = 0;
                    for i in 0..archive.len() {
                        let mut file = archive.by_index(i).unwrap();
                        let name = file.name().to_string();
                        if name.ends_with(".xml") {
                            let dest = xml_dir.join(&name);
                            let mut out = std::fs::File::create(&dest).unwrap();
                            std::io::copy(&mut file, &mut out).unwrap();
                            count += 1;
                        }
                    }
                    println!("    {} XMLs → {}", count, xml_dir.display());
                }
            }
        }
    }

    println!("\nListo. Revisa cfdi_data/");
}
