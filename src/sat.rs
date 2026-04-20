use std::{env, fmt, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    sign::Signer,
    x509::X509,
};
use reqwest::header::{ACCEPT, CACHE_CONTROL, CONTENT_TYPE, HeaderMap, HeaderValue};
use roxmltree::Document;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tokio::{fs, time::sleep};

const AUTH_URL: &str =
    "https://cfdidescargamasivasolicitud.clouda.sat.gob.mx/Autenticacion/Autenticacion.svc";
const REQUEST_URL: &str =
    "https://cfdidescargamasivasolicitud.clouda.sat.gob.mx/SolicitaDescargaService.svc";
const VERIFY_URL: &str =
    "https://cfdidescargamasivasolicitud.clouda.sat.gob.mx/VerificaSolicitudDescargaService.svc";
const DOWNLOAD_URL: &str = "https://cfdidescargamasiva.clouda.sat.gob.mx/DescargaMasivaService.svc";

const NS_SOAP: &str = "http://schemas.xmlsoap.org/soap/envelope/";
const NS_WSSE: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd";
const NS_WSU: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd";
const NS_DSIG: &str = "http://www.w3.org/2000/09/xmldsig#";
const NS_SAT: &str = "http://DescargaMasivaTerceros.sat.gob.mx";
const NS_AUTH: &str = "http://DescargaMasivaTerceros.gob.mx";

#[derive(Debug, Deserialize)]
pub struct CfdiDownloadRequest {
    pub cer_path: Option<String>,
    pub key_path: Option<String>,
    pub key_password: Option<String>,
    pub rfc: Option<String>,
    #[serde(default = "default_download_type")]
    pub download_type: DownloadType,
    #[serde(default = "default_request_type")]
    pub request_type: RequestType,
    pub start: Option<String>,
    pub end: Option<String>,
    pub output_dir: Option<String>,
    pub poll_seconds: Option<u64>,
    pub max_attempts: Option<u32>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadType {
    Issued,
    Received,
}

impl DownloadType {
    fn env_value(self) -> &'static str {
        match self {
            DownloadType::Issued => "issued",
            DownloadType::Received => "received",
        }
    }
}

fn default_download_type() -> DownloadType {
    DownloadType::Issued
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequestType {
    Xml,
    Metadata,
}

impl RequestType {
    fn sat_value(self) -> &'static str {
        match self {
            RequestType::Xml => "CFDI",
            RequestType::Metadata => "Metadata",
        }
    }
}

fn default_request_type() -> RequestType {
    RequestType::Xml
}

#[derive(Debug, Serialize)]
pub struct CfdiDownloadResponse {
    pub request_id: String,
    pub rfc: String,
    pub download_type: DownloadType,
    pub request_type: RequestType,
    pub output_dir: String,
    pub verify: VerifyResult,
    pub packages: Vec<DownloadedPackage>,
}

#[derive(Debug, Serialize)]
pub struct DownloadedPackage {
    pub package_id: String,
    pub path: String,
    pub cod_estatus: Option<String>,
    pub mensaje: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SolicitudResult {
    pub id_solicitud: Option<String>,
    pub cod_estatus: Option<String>,
    pub mensaje: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyResult {
    pub cod_estatus: Option<String>,
    pub estado_solicitud: Option<String>,
    pub codigo_estado_solicitud: Option<String>,
    pub numero_cfdis: Option<String>,
    pub mensaje: Option<String>,
    pub paquetes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SatErrorBody {
    pub error: String,
}

#[derive(Debug)]
pub enum SatError {
    BadRequest(String),
    Crypto(String),
    Http(String),
    Sat(String),
    Io(String),
    Parse(String),
}

impl SatError {
    pub fn body(&self) -> SatErrorBody {
        SatErrorBody {
            error: self.to_string(),
        }
    }
}

impl fmt::Display for SatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SatError::BadRequest(msg) => write!(f, "{msg}"),
            SatError::Crypto(msg) => write!(f, "{msg}"),
            SatError::Http(msg) => write!(f, "{msg}"),
            SatError::Sat(msg) => write!(f, "{msg}"),
            SatError::Io(msg) => write!(f, "{msg}"),
            SatError::Parse(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for SatError {}

struct SatConfig {
    cer_path: String,
    key_path: String,
    key_password: String,
    rfc: String,
    start: String,
    end: String,
    output_dir: PathBuf,
    poll_seconds: u64,
    max_attempts: u32,
    download_type: DownloadType,
    request_type: RequestType,
}

struct Fiel {
    cert: X509,
    key: PKey<Private>,
    cert_der: Vec<u8>,
}

impl Fiel {
    async fn load(cer_path: &str, key_path: &str, password: &str) -> Result<Self, SatError> {
        let cert_der = fs::read(cer_path)
            .await
            .map_err(|err| SatError::Io(format!("no pude leer SAT_CER_PATH: {err}")))?;
        let key_der = fs::read(key_path)
            .await
            .map_err(|err| SatError::Io(format!("no pude leer SAT_KEY_PATH: {err}")))?;

        let cert = X509::from_der(&cert_der)
            .map_err(|err| SatError::Crypto(format!("certificado FIEL invalido: {err}")))?;
        let key = PKey::private_key_from_pkcs8_passphrase(&key_der, password.as_bytes()).map_err(
            |err| SatError::Crypto(format!("llave FIEL invalida o password incorrecto: {err}")),
        )?;

        Ok(Self {
            cert,
            key,
            cert_der,
        })
    }

    fn cert_base64(&self) -> String {
        BASE64.encode(&self.cert_der)
    }

    fn issuer_name(&self) -> Result<String, SatError> {
        let mut parts = Vec::new();
        for entry in self.cert.issuer_name().entries() {
            let key = entry.object().nid().short_name().unwrap_or("UNKNOWN");
            let value = entry.data().as_utf8().map_err(|err| {
                SatError::Crypto(format!("issuer invalido en certificado: {err}"))
            })?;
            parts.push(format!("{key}={value}"));
        }
        Ok(parts.join(","))
    }

    fn serial_number(&self) -> Result<String, SatError> {
        self.cert
            .serial_number()
            .to_bn()
            .and_then(|bn| bn.to_dec_str())
            .map(|s| s.to_string())
            .map_err(|err| SatError::Crypto(format!("serial invalido en certificado: {err}")))
    }

    fn sign_sha1_base64(&self, value: &str) -> Result<String, SatError> {
        let mut signer = Signer::new(MessageDigest::sha1(), &self.key)
            .map_err(|err| SatError::Crypto(format!("no pude iniciar firma SHA1: {err}")))?;
        signer
            .update(value.as_bytes())
            .map_err(|err| SatError::Crypto(format!("no pude alimentar firma SHA1: {err}")))?;
        let signature = signer
            .sign_to_vec()
            .map_err(|err| SatError::Crypto(format!("no pude firmar con FIEL: {err}")))?;
        Ok(BASE64.encode(signature))
    }
}

pub async fn download_cfdis(
    company_slug: &str,
    request: CfdiDownloadRequest,
) -> Result<CfdiDownloadResponse, SatError> {
    let cfg = build_config(company_slug, request)?;
    let fiel = Fiel::load(&cfg.cer_path, &cfg.key_path, &cfg.key_password).await?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|err| SatError::Http(format!("no pude crear cliente HTTP: {err}")))?;

    let token = authenticate(&client, &fiel).await?;
    let solicitud = submit_request(&client, &fiel, &token, &cfg).await?;
    let request_id = solicitud.id_solicitud.clone().ok_or_else(|| {
        SatError::Sat(format!(
            "SAT no devolvio IdSolicitud: {:?}",
            solicitud.mensaje
        ))
    })?;

    let verify = poll_until_finished(&client, &fiel, &cfg, &request_id).await?;
    fs::create_dir_all(&cfg.output_dir)
        .await
        .map_err(|err| SatError::Io(format!("no pude crear output_dir: {err}")))?;

    let mut packages = Vec::new();
    for package_id in &verify.paquetes {
        let token = authenticate(&client, &fiel).await?;
        let downloaded = download_package(&client, &fiel, &token, &cfg, package_id).await?;
        packages.push(downloaded);
    }

    Ok(CfdiDownloadResponse {
        request_id,
        rfc: cfg.rfc,
        download_type: cfg.download_type,
        request_type: cfg.request_type,
        output_dir: cfg.output_dir.to_string_lossy().to_string(),
        verify,
        packages,
    })
}

fn build_config(company_slug: &str, request: CfdiDownloadRequest) -> Result<SatConfig, SatError> {
    let rfc = value_or_env(request.rfc, "SAT_RFC")?.to_uppercase();
    let start = value_or_env(request.start, "SAT_START")?;
    let end = value_or_env(request.end, "SAT_END")?;
    let output_dir = request.output_dir.unwrap_or_else(|| {
        let stamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
        format!(
            "data/cfdi_downloads/{}/{}/{}/{}",
            safe_path_segment(company_slug),
            rfc,
            request.download_type.env_value(),
            stamp
        )
    });

    Ok(SatConfig {
        cer_path: value_or_env(request.cer_path, "SAT_CER_PATH")?,
        key_path: value_or_env(request.key_path, "SAT_KEY_PATH")?,
        key_password: value_or_env(request.key_password, "SAT_KEY_PASSWORD")?,
        rfc,
        start,
        end,
        output_dir: PathBuf::from(output_dir),
        poll_seconds: request.poll_seconds.unwrap_or(10).max(1),
        max_attempts: request.max_attempts.unwrap_or(180).max(1),
        download_type: request.download_type,
        request_type: request.request_type,
    })
}

fn value_or_env(value: Option<String>, name: &str) -> Result<String, SatError> {
    match value.or_else(|| env::var(name).ok()) {
        Some(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(SatError::BadRequest(format!("falta {name}"))),
    }
}

fn safe_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

async fn authenticate(client: &reqwest::Client, fiel: &Fiel) -> Result<String, SatError> {
    let created = Utc::now();
    let expires = created + ChronoDuration::minutes(5);
    let created = fmt_sat_datetime_millis(created);
    let expires = fmt_sat_datetime_millis(expires);
    let cert = fiel.cert_base64();

    let timestamp = format!(
        r#"<u:Timestamp xmlns:u="{NS_WSU}" u:Id="Timestamp"><u:Created>{created}</u:Created><u:Expires>{expires}</u:Expires></u:Timestamp>"#
    );
    let digest = sha1_base64(&timestamp);
    let signed_info = signed_info_for_auth(&digest);
    let signature = fiel.sign_sha1_base64(&signed_info)?;

    let body = format!(
        r##"<s:Envelope xmlns:s="{NS_SOAP}" xmlns:o="{NS_WSSE}" xmlns:u="{NS_WSU}"><s:Header><o:Security s:mustUnderstand="1"><u:Timestamp u:Id="Timestamp"><u:Created>{created}</u:Created><u:Expires>{expires}</u:Expires></u:Timestamp><o:BinarySecurityToken u:Id="BinarySecurityToken" ValueType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-x509-token-profile-1.0#X509v3" EncodingType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary">{cert}</o:BinarySecurityToken><Signature xmlns="{NS_DSIG}"><SignedInfo><CanonicalizationMethod Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"></CanonicalizationMethod><SignatureMethod Algorithm="http://www.w3.org/2000/09/xmldsig#rsa-sha1"></SignatureMethod><Reference URI="#Timestamp"><Transforms><Transform Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"></Transform></Transforms><DigestMethod Algorithm="http://www.w3.org/2000/09/xmldsig#sha1"></DigestMethod><DigestValue>{digest}</DigestValue></Reference></SignedInfo><SignatureValue>{signature}</SignatureValue><KeyInfo><o:SecurityTokenReference><o:Reference ValueType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-x509-token-profile-1.0#X509v3" URI="#BinarySecurityToken"></o:Reference></o:SecurityTokenReference></KeyInfo></Signature></o:Security></s:Header><s:Body><Autentica xmlns="{NS_AUTH}"></Autentica></s:Body></s:Envelope>"##
    );

    let response = post_soap(
        client,
        AUTH_URL,
        "http://DescargaMasivaTerceros.gob.mx/IAutenticacion/Autentica",
        None,
        body,
    )
    .await?;
    first_text_by_name(&response, "AutenticaResult")
        .ok_or_else(|| SatError::Parse("SAT no devolvio token de autenticacion".into()))
}

async fn submit_request(
    client: &reqwest::Client,
    fiel: &Fiel,
    token: &str,
    cfg: &SatConfig,
) -> Result<SolicitudResult, SatError> {
    let (operation, solicitud_unsigned, action) = match cfg.download_type {
        DownloadType::Issued => (
            "SolicitaDescargaEmitidos",
            solicitud_emitidos_unsigned(cfg),
            "http://DescargaMasivaTerceros.sat.gob.mx/ISolicitaDescargaService/SolicitaDescargaEmitidos",
        ),
        DownloadType::Received => (
            "SolicitaDescargaRecibidos",
            solicitud_recibidos_unsigned(cfg),
            "http://DescargaMasivaTerceros.sat.gob.mx/ISolicitaDescargaService/SolicitaDescargaRecibidos",
        ),
    };
    let parent =
        format!(r#"<des:{operation} xmlns:des="{NS_SAT}">{solicitud_unsigned}</des:{operation}>"#);
    let signature = request_signature(fiel, &parent)?;
    let solicitud = insert_signature_before_close(&solicitud_unsigned, &signature);
    let body = soap_body(format!(r#"<des:{operation}>{solicitud}</des:{operation}>"#));

    let response = post_soap(client, REQUEST_URL, action, Some(token), body).await?;
    let result_name = match cfg.download_type {
        DownloadType::Issued => "SolicitaDescargaEmitidosResult",
        DownloadType::Received => "SolicitaDescargaRecibidosResult",
    };
    let result = first_attrs_by_name(&response, result_name)
        .ok_or_else(|| SatError::Parse("SAT no devolvio resultado de solicitud".into()))?;

    let solicitud = SolicitudResult {
        id_solicitud: attr_from(&result, "IdSolicitud"),
        cod_estatus: attr_from(&result, "CodEstatus"),
        mensaje: attr_from(&result, "Mensaje"),
    };

    if solicitud.cod_estatus.as_deref() != Some("5000") {
        return Err(SatError::Sat(format!(
            "solicitud SAT rechazada: {} {}",
            solicitud.cod_estatus.clone().unwrap_or_default(),
            solicitud.mensaje.clone().unwrap_or_default()
        )));
    }

    Ok(solicitud)
}

async fn poll_until_finished(
    client: &reqwest::Client,
    fiel: &Fiel,
    cfg: &SatConfig,
    request_id: &str,
) -> Result<VerifyResult, SatError> {
    for _ in 0..cfg.max_attempts {
        let token = authenticate(client, fiel).await?;
        let verify = verify_request(client, fiel, &token, &cfg.rfc, request_id).await?;
        match verify.estado_solicitud.as_deref() {
            Some("1") | Some("2") => sleep(std::time::Duration::from_secs(cfg.poll_seconds)).await,
            Some("3") => return Ok(verify),
            Some("5")
                if verify.codigo_estado_solicitud.as_deref() == Some("5004")
                    || verify.cod_estatus.as_deref() == Some("5004") =>
            {
                return Ok(verify);
            }
            Some(other) => {
                return Err(SatError::Sat(format!(
                    "solicitud SAT no terminable, estado {other}: {} {}",
                    verify.codigo_estado_solicitud.clone().unwrap_or_default(),
                    verify.mensaje.clone().unwrap_or_default()
                )));
            }
            None => return Err(SatError::Parse("verificacion sin EstadoSolicitud".into())),
        }
    }

    Err(SatError::Sat(format!(
        "la solicitud SAT no termino despues de {} intentos",
        cfg.max_attempts
    )))
}

async fn verify_request(
    client: &reqwest::Client,
    fiel: &Fiel,
    token: &str,
    rfc: &str,
    request_id: &str,
) -> Result<VerifyResult, SatError> {
    let solicitud_unsigned = format!(
        r#"<des:solicitud IdSolicitud="{request_id}" RfcSolicitante="{rfc}"></des:solicitud>"#
    );
    let parent = format!(
        r#"<des:VerificaSolicitudDescarga xmlns:des="{NS_SAT}">{solicitud_unsigned}</des:VerificaSolicitudDescarga>"#
    );
    let signature = request_signature(fiel, &parent)?;
    let solicitud = insert_signature_before_close(&solicitud_unsigned, &signature);
    let body = soap_body(format!(
        r#"<des:VerificaSolicitudDescarga>{solicitud}</des:VerificaSolicitudDescarga>"#
    ));

    let response = post_soap(
        client,
        VERIFY_URL,
        "http://DescargaMasivaTerceros.sat.gob.mx/IVerificaSolicitudDescargaService/VerificaSolicitudDescarga",
        Some(token),
        body,
    )
    .await?;
    parse_verify_result(&response)
}

async fn download_package(
    client: &reqwest::Client,
    fiel: &Fiel,
    token: &str,
    cfg: &SatConfig,
    package_id: &str,
) -> Result<DownloadedPackage, SatError> {
    let solicitud_unsigned = format!(
        r#"<des:peticionDescarga IdPaquete="{package_id}" RfcSolicitante="{}"></des:peticionDescarga>"#,
        cfg.rfc
    );
    let parent = format!(
        r#"<des:PeticionDescargaMasivaTercerosEntrada xmlns:des="{NS_SAT}">{solicitud_unsigned}</des:PeticionDescargaMasivaTercerosEntrada>"#
    );
    let signature = request_signature(fiel, &parent)?;
    let solicitud = insert_signature_before_close(&solicitud_unsigned, &signature);
    let body = soap_body(format!(
        r#"<des:PeticionDescargaMasivaTercerosEntrada>{solicitud}</des:PeticionDescargaMasivaTercerosEntrada>"#
    ));
    let response = post_soap(
        client,
        DOWNLOAD_URL,
        "http://DescargaMasivaTerceros.sat.gob.mx/IDescargaMasivaTercerosService/Descargar",
        Some(token),
        body,
    )
    .await?;
    let package_b64 = first_text_by_name(&response, "Paquete")
        .ok_or_else(|| SatError::Parse("SAT no devolvio paquete".into()))?;
    let bytes = BASE64
        .decode(package_b64)
        .map_err(|err| SatError::Parse(format!("paquete SAT no venia en base64 valido: {err}")))?;
    let path = cfg.output_dir.join(format!("{package_id}.zip"));
    fs::write(&path, bytes)
        .await
        .map_err(|err| SatError::Io(format!("no pude escribir paquete SAT: {err}")))?;

    let respuesta = first_attrs_by_name(&response, "respuesta");
    Ok(DownloadedPackage {
        package_id: package_id.to_string(),
        path: path.to_string_lossy().to_string(),
        cod_estatus: respuesta
            .as_ref()
            .and_then(|attrs| attr_from(attrs, "CodEstatus")),
        mensaje: respuesta
            .as_ref()
            .and_then(|attrs| attr_from(attrs, "Mensaje")),
    })
}

fn solicitud_emitidos_unsigned(cfg: &SatConfig) -> String {
    let mut attrs = solicitud_attrs(cfg);
    attrs.push(format!(r#"RfcEmisor="{}""#, cfg.rfc));
    format!(
        r#"<des:solicitud {}><des:RfcReceptores><des:RfcReceptor></des:RfcReceptor></des:RfcReceptores></des:solicitud>"#,
        attrs.join(" ")
    )
}

fn solicitud_recibidos_unsigned(cfg: &SatConfig) -> String {
    let mut attrs = solicitud_attrs(cfg);
    attrs.push(format!(r#"RfcReceptor="{}""#, cfg.rfc));
    if cfg.download_type == DownloadType::Received && cfg.request_type == RequestType::Xml {
        attrs.push(r#"EstadoComprobante="Vigente""#.to_string());
    }
    format!(r#"<des:solicitud {}></des:solicitud>"#, attrs.join(" "))
}

fn solicitud_attrs(cfg: &SatConfig) -> Vec<String> {
    vec![
        format!(r#"RfcSolicitante="{}""#, cfg.rfc),
        format!(r#"FechaFinal="{}""#, cfg.end),
        format!(r#"FechaInicial="{}""#, cfg.start),
        format!(r#"TipoSolicitud="{}""#, cfg.request_type.sat_value()),
    ]
}

fn request_signature(fiel: &Fiel, parent_c14n: &str) -> Result<String, SatError> {
    let digest = sha1_base64(parent_c14n);
    let signed_info = signed_info_for_request(&digest);
    let signature = fiel.sign_sha1_base64(&signed_info)?;
    let cert = fiel.cert_base64();
    let issuer = fiel.issuer_name()?;
    let serial = fiel.serial_number()?;
    Ok(format!(
        r#"<Signature xmlns="{NS_DSIG}"><SignedInfo><CanonicalizationMethod Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"></CanonicalizationMethod><SignatureMethod Algorithm="http://www.w3.org/2000/09/xmldsig#rsa-sha1"></SignatureMethod><Reference><Transforms><Transform Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"></Transform></Transforms><DigestMethod Algorithm="http://www.w3.org/2000/09/xmldsig#sha1"></DigestMethod><DigestValue>{digest}</DigestValue></Reference></SignedInfo><SignatureValue>{signature}</SignatureValue><KeyInfo><X509Data><X509IssuerSerial><X509IssuerName>{issuer}</X509IssuerName><X509SerialNumber>{serial}</X509SerialNumber></X509IssuerSerial><X509Certificate>{cert}</X509Certificate></X509Data></KeyInfo></Signature>"#
    ))
}

fn insert_signature_before_close(solicitud: &str, signature: &str) -> String {
    if let Some(idx) = solicitud.rfind("</des:solicitud>") {
        let mut out = String::with_capacity(solicitud.len() + signature.len());
        out.push_str(&solicitud[..idx]);
        out.push_str(signature);
        out.push_str(&solicitud[idx..]);
        out
    } else if let Some(idx) = solicitud.rfind("</des:peticionDescarga>") {
        let mut out = String::with_capacity(solicitud.len() + signature.len());
        out.push_str(&solicitud[..idx]);
        out.push_str(signature);
        out.push_str(&solicitud[idx..]);
        out
    } else {
        solicitud.to_string()
    }
}

fn signed_info_for_auth(digest: &str) -> String {
    format!(
        r##"<SignedInfo xmlns="{NS_DSIG}"><CanonicalizationMethod Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"></CanonicalizationMethod><SignatureMethod Algorithm="http://www.w3.org/2000/09/xmldsig#rsa-sha1"></SignatureMethod><Reference URI="#Timestamp"><Transforms><Transform Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"></Transform></Transforms><DigestMethod Algorithm="http://www.w3.org/2000/09/xmldsig#sha1"></DigestMethod><DigestValue>{digest}</DigestValue></Reference></SignedInfo>"##
    )
}

fn signed_info_for_request(digest: &str) -> String {
    format!(
        r#"<SignedInfo xmlns="{NS_DSIG}"><CanonicalizationMethod Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"></CanonicalizationMethod><SignatureMethod Algorithm="http://www.w3.org/2000/09/xmldsig#rsa-sha1"></SignatureMethod><Reference><Transforms><Transform Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"></Transform></Transforms><DigestMethod Algorithm="http://www.w3.org/2000/09/xmldsig#sha1"></DigestMethod><DigestValue>{digest}</DigestValue></Reference></SignedInfo>"#
    )
}

fn soap_body(inner: String) -> String {
    format!(
        r#"<s:Envelope xmlns:des="{NS_SAT}" xmlns:s="{NS_SOAP}"><s:Header></s:Header><s:Body>{inner}</s:Body></s:Envelope>"#
    )
}

async fn post_soap(
    client: &reqwest::Client,
    url: &str,
    soap_action: &str,
    token: Option<&str>,
    body: String,
) -> Result<String, SatError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/xml;charset=\"utf-8\""),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("text/xml"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(
        "SOAPAction",
        HeaderValue::from_str(soap_action)
            .map_err(|err| SatError::Http(format!("SOAPAction invalido: {err}")))?,
    );
    if let Some(token) = token {
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("WRAP access_token=\"{token}\""))
                .map_err(|err| SatError::Http(format!("token SAT invalido: {err}")))?,
        );
    }

    let response = client
        .post(url)
        .headers(headers)
        .body(body)
        .send()
        .await
        .map_err(|err| SatError::Http(format!("fallo HTTP contra SAT: {err}")))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| SatError::Http(format!("no pude leer respuesta SAT: {err}")))?;

    if !status.is_success() {
        let fault = first_text_by_name(&text, "faultstring").unwrap_or_else(|| text.clone());
        return Err(SatError::Sat(format!("SAT HTTP {status}: {fault}")));
    }

    Ok(text)
}

fn parse_verify_result(xml: &str) -> Result<VerifyResult, SatError> {
    let doc = Document::parse(xml)
        .map_err(|err| SatError::Parse(format!("respuesta XML SAT invalida: {err}")))?;
    let result = first_node_by_name_in_doc(&doc, "VerificaSolicitudDescargaResult")
        .ok_or_else(|| SatError::Parse("SAT no devolvio resultado de verificacion".into()))?;
    let paquetes = result
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "IdsPaquetes")
        .filter_map(|node| node.text().map(ToOwned::to_owned))
        .collect();

    Ok(VerifyResult {
        cod_estatus: result.attribute("CodEstatus").map(ToOwned::to_owned),
        estado_solicitud: result.attribute("EstadoSolicitud").map(ToOwned::to_owned),
        codigo_estado_solicitud: result
            .attribute("CodigoEstadoSolicitud")
            .map(ToOwned::to_owned),
        numero_cfdis: result.attribute("NumeroCFDIs").map(ToOwned::to_owned),
        mensaje: result.attribute("Mensaje").map(ToOwned::to_owned),
        paquetes,
    })
}

fn first_text_by_name(xml: &str, name: &str) -> Option<String> {
    let doc = Document::parse(xml).ok()?;
    first_node_by_name_in_doc(&doc, name).and_then(|node| node.text().map(ToOwned::to_owned))
}

fn first_attrs_by_name(xml: &str, name: &str) -> Option<Vec<(String, String)>> {
    let doc = Document::parse(xml).ok()?;
    let node = first_node_by_name_in_doc(&doc, name)?;
    Some(
        node.attributes()
            .map(|attr| (attr.name().to_string(), attr.value().to_string()))
            .collect(),
    )
}

fn first_node_by_name_in_doc<'a, 'input>(
    doc: &'a Document<'input>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    doc.descendants()
        .find(|node| node.is_element() && node.tag_name().name() == name)
}

fn attr_from(attrs: &[(String, String)], name: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
}

fn sha1_base64(value: &str) -> String {
    let digest = Sha1::digest(value.as_bytes());
    BASE64.encode(digest)
}

fn fmt_sat_datetime_millis(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_type_maps_to_sat_values() {
        assert_eq!(RequestType::Xml.sat_value(), "CFDI");
        assert_eq!(RequestType::Metadata.sat_value(), "Metadata");
    }

    #[test]
    fn received_xml_forces_vigente_status() {
        let cfg = SatConfig {
            cer_path: "cer".into(),
            key_path: "key".into(),
            key_password: "pw".into(),
            rfc: "XAXX010101000".into(),
            start: "2026-01-01T00:00:00".into(),
            end: "2026-01-31T23:59:59".into(),
            output_dir: PathBuf::from("out"),
            poll_seconds: 1,
            max_attempts: 1,
            download_type: DownloadType::Received,
            request_type: RequestType::Xml,
        };

        let xml = solicitud_recibidos_unsigned(&cfg);
        assert!(xml.contains(r#"EstadoComprobante="Vigente""#));
        assert!(xml.contains(r#"RfcReceptor="XAXX010101000""#));
    }

    #[tokio::test]
    #[ignore = "requires real SAT credentials and network access"]
    async fn real_sat_metadata_download_from_env() {
        let request = CfdiDownloadRequest {
            cer_path: None,
            key_path: None,
            key_password: None,
            rfc: None,
            download_type: match env::var("SAT_TEST_DOWNLOAD_TYPE")
                .unwrap_or_else(|_| "received".to_string())
                .as_str()
            {
                "issued" => DownloadType::Issued,
                _ => DownloadType::Received,
            },
            request_type: match env::var("SAT_TEST_REQUEST_TYPE")
                .unwrap_or_else(|_| "metadata".to_string())
                .as_str()
            {
                "xml" => RequestType::Xml,
                _ => RequestType::Metadata,
            },
            start: Some(
                env::var("SAT_TEST_START").unwrap_or_else(|_| "2026-01-01T00:00:00".to_string()),
            ),
            end: Some(
                env::var("SAT_TEST_END").unwrap_or_else(|_| "2026-01-01T23:59:59".to_string()),
            ),
            output_dir: Some(
                env::var("SAT_TEST_OUTPUT_DIR")
                    .unwrap_or_else(|_| "/tmp/alfredodev-sat-test".to_string()),
            ),
            poll_seconds: Some(
                env::var("SAT_TEST_POLL_SECONDS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(5),
            ),
            max_attempts: Some(
                env::var("SAT_TEST_MAX_ATTEMPTS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(6),
            ),
        };

        let result = download_cfdis("sat-test", request).await;
        println!("SAT result: {result:?}");
        assert!(result.is_ok(), "SAT download failed: {result:?}");
    }
}
