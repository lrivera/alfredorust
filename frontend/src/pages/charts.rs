//! Small dependency-free SVG charts (line+area, donut) used by the finance
//! screens, reimplemented in Rust from the v1 React components. Each builder
//! returns an SVG/HTML string to drop into an element via `inner_html`.

/// Income (green) vs expense (rose) area+line chart over labeled buckets
/// `(label, income, expense)`. Mirrors the v1 `LineChart` (W=560, H=130).
pub(crate) fn line_area_chart(data: &[(String, f64, f64)]) -> String {
    if data.is_empty() {
        return r#"<div style="display:flex;align-items:center;justify-content:center;height:100%;color:#94a3b8;font-size:13px">Sin datos</div>"#.to_string();
    }
    let (w, h, pb) = (560.0_f64, 130.0_f64, 28.0_f64);
    let max = data
        .iter()
        .flat_map(|(_, i, e)| [*i, *e])
        .fold(1.0_f64, f64::max);
    let n = data.len();
    let xstep = if n > 1 { w / (n as f64 - 1.0) } else { w };
    let xof = |i: usize| i as f64 * xstep;
    let yof = |v: f64| h - (v / max) * h;
    let line_path = |key: usize| {
        data.iter()
            .enumerate()
            .map(|(i, t)| {
                let v = if key == 0 { t.1 } else { t.2 };
                format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, xof(i), yof(v))
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let area_path = |key: usize| {
        format!(
            "{} L{:.1},{:.1} L0,{:.1} Z",
            line_path(key),
            xof(n - 1),
            yof(0.0),
            yof(0.0)
        )
    };
    let every = ((n as f64 / 7.0).ceil() as usize).max(1);
    let grid: String = [0.0, 0.5, 1.0]
        .iter()
        .map(|t| {
            let y = yof(max * t);
            format!(r##"<line x1="0" x2="{w}" y1="{y:.1}" y2="{y:.1}" stroke="currentColor" stroke-opacity="0.12" stroke-width="1"/>"##)
        })
        .collect();
    let labels: String = data
        .iter()
        .enumerate()
        .filter(|(i, _)| i % every == 0 || *i == n - 1)
        .map(|(i, t)| {
            let anchor = if i == 0 {
                "start"
            } else if i == n - 1 {
                "end"
            } else {
                "middle"
            };
            let label = t.0.get(2..).unwrap_or(&t.0);
            format!(
                r##"<text x="{:.1}" y="{:.1}" text-anchor="{anchor}" font-size="10" fill="#94a3b8">{label}</text>"##,
                xof(i),
                h + pb - 4.0
            )
        })
        .collect();
    format!(
        r##"<svg viewBox="0 0 {w} {hpb}" style="width:100%;height:100%">
  <defs>
    <linearGradient id="chartI" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color="#10b981" stop-opacity=".15"/><stop offset="100%" stop-color="#10b981" stop-opacity="0"/></linearGradient>
    <linearGradient id="chartE" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color="#f43f5e" stop-opacity=".12"/><stop offset="100%" stop-color="#f43f5e" stop-opacity="0"/></linearGradient>
  </defs>
  {grid}
  <path d="{ae}" fill="url(#chartE)"/>
  <path d="{ai}" fill="url(#chartI)"/>
  <path d="{le}" fill="none" stroke="#f43f5e" stroke-width="2" stroke-linejoin="round"/>
  <path d="{li}" fill="none" stroke="#10b981" stroke-width="2" stroke-linejoin="round"/>
  {labels}
</svg>"##,
        hpb = h + pb,
        ae = area_path(1),
        ai = area_path(0),
        le = line_path(1),
        li = line_path(0),
    )
}

/// Donut from `(label, value, css_color)` segments, with a centered total.
/// Mirrors the v1 `DonutChart` (144×144, r=54, stroke 20).
pub(crate) fn donut(segments: &[(String, f64, String)], center_value: &str, center_label: &str) -> String {
    let (cx, cy, r, sw) = (72.0_f64, 72.0_f64, 54.0_f64, 20.0_f64);
    let circ = 2.0 * std::f64::consts::PI * r;
    let total: f64 = segments.iter().map(|(_, v, _)| *v).sum::<f64>().max(1.0);
    let mut cum = 0.0_f64;
    let arcs: String = segments
        .iter()
        .filter(|(_, v, _)| *v > 0.0)
        .map(|(_, v, color)| {
            let frac = v / total;
            let dash = frac * circ;
            let rot = (cum / total) * 360.0 - 90.0;
            cum += v;
            format!(
                r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="none" stroke="{color}" stroke-width="{sw}" stroke-dasharray="{dash:.2} {gap:.2}" transform="rotate({rot:.2} {cx} {cy})"/>"#,
                gap = circ - dash
            )
        })
        .collect();
    format!(
        r##"<svg viewBox="0 0 144 144" style="width:100%;height:100%">
  <circle cx="{cx}" cy="{cy}" r="{r}" fill="none" stroke="currentColor" stroke-opacity="0.12" stroke-width="{sw}"/>
  {arcs}
  <text x="{cx}" y="{ty:.0}" text-anchor="middle" style="font-size:18px;font-weight:700;fill:currentColor">{center_value}</text>
  <text x="{cx}" y="{sy:.0}" text-anchor="middle" style="font-size:10px;fill:#94a3b8">{center_label}</text>
</svg>"##,
        ty = cy - 7.0,
        sy = cy + 12.0,
    )
}
