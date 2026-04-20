// Parse CFDI 3.3/4.0 XML and upsert into MongoDB by UUID.

use anyhow::{Context, Result, bail};
use bson::{Bson, doc};
use mongodb::Collection;
use roxmltree::{Document, Node};
use std::io::Read;
use zip::ZipArchive;

/// Key fields extracted from a CFDI, returned after each successful import.
#[derive(Debug, Clone)]
pub struct ImportedCfdi {
    pub uuid: String,
    /// "I" = Ingreso, "E" = Egreso, "T" = Traslado, "N" = Nómina, "P" = Pago
    pub tipo_de_comprobante: String,
    pub total: String,
    pub fecha: String,
    pub emisor_rfc: String,
    pub emisor_nombre: String,
    pub receptor_rfc: String,
    pub receptor_nombre: String,
}

/// Extract and import all CFDI XML files from a ZIP. Returns imported CFDIs.
pub async fn import_zip(
    collection: &Collection<bson::Document>,
    company_id: &str,
    zip_bytes: &[u8],
) -> Result<Vec<ImportedCfdi>> {
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(cursor).context("Opening ZIP")?;

    let mut xml_files: Vec<(String, String)> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("Reading ZIP entry")?;
        let name = entry.name().to_lowercase();
        if !name.ends_with(".xml") {
            continue;
        }
        let mut xml = String::new();
        entry
            .read_to_string(&mut xml)
            .with_context(|| format!("Reading {name} from ZIP"))?;
        xml_files.push((name, xml));
    }

    let mut imported = Vec::new();
    for (name, xml) in xml_files {
        match import_xml(collection, company_id, &xml).await {
            Ok(cfdi) => imported.push(cfdi),
            Err(e) => eprintln!("[cfdi] skip {name}: {e}"),
        }
    }

    Ok(imported)
}

/// Parse a single CFDI XML string and upsert into MongoDB (keyed by UUID).
pub async fn import_xml(
    collection: &Collection<bson::Document>,
    company_id: &str,
    xml: &str,
) -> Result<ImportedCfdi> {
    let xml = xml.strip_prefix('\u{FEFF}').unwrap_or(xml);
    let xml_doc = Document::parse(xml).context("Parsing CFDI XML")?;
    let (uuid, mut bson_doc, summary) = parse_cfdi(&xml_doc)?;

    bson_doc.insert("company_id", company_id);

    collection
        .update_one(doc! { "uuid": &uuid }, doc! { "$set": &bson_doc })
        .upsert(true)
        .await
        .context("MongoDB upsert")?;

    Ok(summary)
}

fn parse_cfdi(doc: &Document) -> Result<(String, bson::Document, ImportedCfdi)> {
    let root = doc.root_element();
    if root.tag_name().name() != "Comprobante" {
        bail!("Root element is not cfdi:Comprobante");
    }

    let tfd = descendent(root, "TimbreFiscalDigital")
        .context("TimbreFiscalDigital not found — maybe not a timbrado CFDI")?;
    let uuid = tfd
        .attribute("UUID")
        .context("UUID missing in TimbreFiscalDigital")?
        .to_lowercase();

    let comprobante = doc! {
        "version":           root.attribute("Version").unwrap_or(""),
        "folio":             root.attribute("Folio").unwrap_or(""),
        "fecha":             root.attribute("Fecha").unwrap_or(""),
        "formaPago":         root.attribute("FormaPago").unwrap_or(""),
        "metodoPago":        root.attribute("MetodoPago").unwrap_or(""),
        "tipoDeComprobante": root.attribute("TipoDeComprobante").unwrap_or(""),
        "exportacion":       root.attribute("Exportacion").unwrap_or(""),
        "moneda":            root.attribute("Moneda").unwrap_or(""),
        "subTotal":          root.attribute("SubTotal").unwrap_or(""),
        "total":             root.attribute("Total").unwrap_or(""),
        "lugarExpedicion":   root.attribute("LugarExpedicion").unwrap_or(""),
        "noCertificado":     root.attribute("NoCertificado").unwrap_or(""),
        "sello":             root.attribute("Sello").unwrap_or(""),
        "certificado":       root.attribute("Certificado").unwrap_or(""),
    };

    let tipo = root.attribute("TipoDeComprobante").unwrap_or("").to_string();
    let total_str = root.attribute("Total").unwrap_or("0").to_string();
    let fecha_str = root.attribute("Fecha").unwrap_or("").to_string();

    let emisor_node = child(root, "Emisor");
    let emisor_rfc = emisor_node.and_then(|n| n.attribute("Rfc")).unwrap_or("").to_string();
    let emisor_nombre = emisor_node.and_then(|n| n.attribute("Nombre")).unwrap_or("").to_string();
    let emisor = doc! {
        "rfc":          &emisor_rfc,
        "nombre":       &emisor_nombre,
        "regimenFiscal":emisor_node.and_then(|n| n.attribute("RegimenFiscal")).unwrap_or(""),
    };

    let receptor_node = child(root, "Receptor");
    let receptor_rfc = receptor_node.and_then(|n| n.attribute("Rfc")).unwrap_or("").to_string();
    let receptor_nombre = receptor_node.and_then(|n| n.attribute("Nombre")).unwrap_or("").to_string();
    let receptor = doc! {
        "rfc":           &receptor_rfc,
        "nombre":        &receptor_nombre,
        "domicilioFiscal":receptor_node.and_then(|n| n.attribute("DomicilioFiscalReceptor")).unwrap_or(""),
        "regimenFiscal": receptor_node.and_then(|n| n.attribute("RegimenFiscalReceptor")).unwrap_or(""),
        "usoCFDI":       receptor_node.and_then(|n| n.attribute("UsoCFDI")).unwrap_or(""),
    };

    let conceptos: Vec<Bson> = child(root, "Conceptos")
        .map(|cn| {
            cn.children()
                .filter(|n| n.is_element() && n.tag_name().name() == "Concepto")
                .map(|c| Bson::Document(parse_concepto(c)))
                .collect()
        })
        .unwrap_or_default();

    let impuestos = child(root, "Impuestos")
        .map(parse_impuestos)
        .unwrap_or_default();

    let timbre = doc! {
        "version":        tfd.attribute("Version").unwrap_or(""),
        "uuid":           &uuid,
        "fechaTimbrado":  tfd.attribute("FechaTimbrado").unwrap_or(""),
        "rfcProvCertif":  tfd.attribute("RfcProvCertif").unwrap_or(""),
        "noCertificadoSAT": tfd.attribute("NoCertificadoSAT").unwrap_or(""),
        "selloCFD":       tfd.attribute("SelloCFD").unwrap_or(""),
        "selloSAT":       tfd.attribute("SelloSAT").unwrap_or(""),
    };

    let out = doc! {
        "uuid":                 &uuid,
        "comprobante":          comprobante,
        "emisor":               emisor,
        "receptor":             receptor,
        "conceptos":            conceptos,
        "impuestos":            impuestos,
        "timbreFiscalDigital":  timbre,
    };

    let summary = ImportedCfdi {
        uuid: uuid.clone(),
        tipo_de_comprobante: tipo,
        total: total_str,
        fecha: fecha_str,
        emisor_rfc,
        emisor_nombre,
        receptor_rfc,
        receptor_nombre,
    };

    Ok((uuid, out, summary))
}

fn parse_concepto(node: Node) -> bson::Document {
    let traslados: Vec<Bson> = child(node, "Impuestos")
        .and_then(|imp| child(imp, "Traslados"))
        .map(|tr| {
            tr.children()
                .filter(|n| n.is_element() && n.tag_name().name() == "Traslado")
                .map(|t| Bson::Document(parse_traslado(t)))
                .collect()
        })
        .unwrap_or_default();

    let mut d = doc! {
        "claveProdServ":    node.attribute("ClaveProdServ").unwrap_or(""),
        "claveUnidad":      node.attribute("ClaveUnidad").unwrap_or(""),
        "cantidad":         node.attribute("Cantidad").unwrap_or(""),
        "noIdentificacion": node.attribute("NoIdentificacion").unwrap_or(""),
        "descripcion":      node.attribute("Descripcion").unwrap_or(""),
        "valorUnitario":    node.attribute("ValorUnitario").unwrap_or(""),
        "importe":          node.attribute("Importe").unwrap_or(""),
        "objetoImp":        node.attribute("ObjetoImp").unwrap_or(""),
    };
    if !traslados.is_empty() {
        d.insert("traslados", traslados);
    }
    d
}

fn parse_traslado(node: Node) -> bson::Document {
    doc! {
        "base":        node.attribute("Base").unwrap_or(""),
        "impuesto":    node.attribute("Impuesto").unwrap_or(""),
        "tipoFactor":  node.attribute("TipoFactor").unwrap_or(""),
        "tasaOCuota":  node.attribute("TasaOCuota").unwrap_or(""),
        "importe":     node.attribute("Importe").unwrap_or(""),
    }
}

fn parse_impuestos(node: Node) -> bson::Document {
    let traslados: Vec<Bson> = child(node, "Traslados")
        .map(|tr| {
            tr.children()
                .filter(|n| n.is_element() && n.tag_name().name() == "Traslado")
                .map(|t| Bson::Document(parse_traslado(t)))
                .collect()
        })
        .unwrap_or_default();

    let mut d = doc! {
        "totalImpuestosTrasladados": node.attribute("TotalImpuestosTrasladados").unwrap_or(""),
    };
    if !traslados.is_empty() {
        d.insert("traslados", traslados);
    }
    d
}

fn child<'a, 'input>(node: Node<'a, 'input>, local: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == local)
}

fn descendent<'a, 'input>(node: Node<'a, 'input>, local: &str) -> Option<Node<'a, 'input>> {
    node.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == local)
}
