//! NUMEROPDF: native PDF operations, backed by pinned PdfCraft crates.
#![forbid(unsafe_code)]
pub mod batch;
pub mod layout;
pub mod marker;
use pdfcraft_cos::{Document, SaveOptions, write_full};
use pdfcraft_edit::{AddedText, Align, Content, Family};
use pdfcraft_render::{PageInfo, PageRenderer, RenderConfig, RenderRequest};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path, sync::Arc};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
pub type Result<T> = std::result::Result<T, Error>;
fn err(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Area {
    pub fn overlaps(self, other: Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
    pub fn valid(self, page: &PageInfo) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.x >= 0.0
            && self.y >= 0.0
            && self.width > 0.0
            && self.height > 0.0
            && self.x + self.width <= page.width as f64 + 0.01
            && self.y + self.height <= page.height as f64 + 0.01
    }
}
#[derive(Clone, Debug)]
pub struct Candidate {
    pub page: usize,
    pub area: Area,
    pub text: String,
    pub text_rect: [f64; 4],
    /// Index into editable text lines (not paragraph blocks).
    pub line: Option<usize>,
    pub source_size: Option<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    Existing,
    Manual,
    PurpleMarker,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Numbering {
    /// Zero-based, inclusive page interval. UI converts from one-based.
    pub first_page: usize,
    pub last_page: usize,
    pub start: u32,
    pub digits: usize,
    pub show_total: bool,
    pub total_is_last: bool,
    pub prefix: String,
    pub separator: String,
    pub mode: Mode,
    pub area: Area,
    pub size: f64,
    pub auto_fit: bool,
    pub inherit_size: bool,
    /// Restrict automatic replacement to the selected area.
    pub restrict_area: bool,
    /// 0 Helvetica, 1 Times, 2 Courier.
    pub font: usize,
    pub bold: bool,
    pub italic: bool,
    pub color: [u8; 3],
    /// Marker replacement is a visual white cover, never represented as secure deletion.
    pub allow_marker_cover: bool,
    pub marker_pattern: marker::Pattern,
}
impl Default for Numbering {
    fn default() -> Self {
        Self {
            first_page: 1,
            last_page: 1,
            start: 1,
            digits: 2,
            show_total: true,
            total_is_last: false,
            prefix: String::new(),
            separator: "/".into(),
            mode: Mode::Existing,
            area: Area {
                x: 780.0,
                y: 533.0,
                width: 44.0,
                height: 22.0,
            },
            size: 14.0,
            auto_fit: true,
            inherit_size: true,
            restrict_area: false,
            font: 0,
            bold: true,
            italic: false,
            color: [0; 3],
            allow_marker_cover: false,
            marker_pattern: marker::Pattern::default(),
        }
    }
}
impl Numbering {
    pub fn label(&self, offset: usize, count: usize) -> Result<String> {
        if self.digits > 9 || count == 0 || offset >= count {
            return Err(err("Sequência inválida"));
        }
        let n = self
            .start
            .checked_add(u32::try_from(offset).map_err(err)?)
            .ok_or_else(|| err("Número excede o limite"))?;
        let total = if self.total_is_last {
            self.start
                .checked_add(u32::try_from(count - 1).map_err(err)?)
                .ok_or_else(|| err("Total excede o limite"))?
        } else {
            u32::try_from(count).map_err(err)?
        };
        let mut s = format!("{}{:0width$}", self.prefix, n, width = self.digits);
        if self.show_total {
            s.push_str(&format!(
                "{}{:0width$}",
                self.separator,
                total,
                width = self.digits
            ));
        }
        Ok(s)
    }
}

pub fn is_page_number(text: &str) -> bool {
    let Some((a, b)) = text.trim().split_once('/') else {
        return false;
    };
    let (a, b) = (a.trim(), b.trim());
    !a.is_empty()
        && !b.is_empty()
        && a.len() <= 9
        && b.len() <= 9
        && a.bytes().all(|c| c.is_ascii_digit())
        && b.bytes().all(|c| c.is_ascii_digit())
}

fn same_number_position(a: [f64; 4], b: [f64; 4]) -> bool {
    let intersection =
        (a[2].min(b[2]) - a[0].max(b[0])).max(0.0) * (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let area = ((a[2] - a[0]) * (a[3] - a[1])).min((b[2] - b[0]) * (b[3] - b[1]));
    area > 0.0 && intersection / area > 0.5
}

pub struct Pdf {
    pub bytes: Arc<Vec<u8>>,
    pub pages: Vec<PageInfo>,
}
impl Pdf {
    pub fn open(path: &Path) -> Result<Self> {
        if std::fs::metadata(path).map_err(err)?.len() > 512 * 1024 * 1024 {
            return Err(err("Limite de abertura: 512 MB"));
        }
        Self::from_bytes(std::fs::read(path).map_err(err)?)
    }
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        let bytes = Arc::new(bytes);
        let info = pdfcraft_render::inspect(bytes.clone(), None).map_err(err)?;
        if info.encrypted {
            return Err(err("PDF protegido: abra uma cópia sem senha nesta versão"));
        }
        if info.pages.is_empty() || info.pages.len() > 10_000 {
            return Err(err("Quantidade de páginas inválida (máximo 10.000)"));
        }
        Ok(Self {
            bytes,
            pages: info.pages,
        })
    }
    fn document(&self) -> Result<Document> {
        Document::open(self.bytes.clone()).map_err(err)
    }
    pub fn render(&self, page: usize, scale: f32) -> Result<image::RgbaImage> {
        if page >= self.pages.len() || !scale.is_finite() || scale <= 0.0 {
            return Err(err("Página ou escala inválida"));
        }
        let mut r = PageRenderer::new(
            self.bytes.clone(),
            RenderConfig {
                reject_oversize: true,
                ..Default::default()
            },
        );
        let out = r.render(RenderRequest {
            page,
            scale,
            ..Default::default()
        });
        if let Some(e) = out.error {
            return Err(err(e));
        }
        if !out.warnings.is_empty() {
            return Err(err(format!("Renderização incompleta: {:?}", out.warnings)));
        }
        image::RgbaImage::from_raw(out.width, out.height, out.rgba.to_vec())
            .ok_or_else(|| err("Raster inválido"))
    }
    /// Inspect once on import, outside the UI thread, excluding unnumbered covers.
    pub fn numbering_range(&self) -> Result<Option<(usize, usize)>> {
        let doc = self.document()?;
        let mut first = None;
        let mut last = 0;
        for page in 0..self.pages.len() {
            if !self.candidates_in(&doc, page)?.is_empty() {
                first.get_or_insert(page);
                last = page;
            }
        }
        Ok(first.map(|first| (first, last)))
    }
    pub fn candidates(&self, page: usize) -> Result<Vec<Candidate>> {
        let doc = self.document()?;
        self.candidates_in(&doc, page)
    }
    fn candidates_in(&self, doc: &Document, page: usize) -> Result<Vec<Candidate>> {
        let info = self
            .pages
            .get(page)
            .ok_or_else(|| err("Página inexistente"))?;
        let rules = pdfcraft_edit::page_rules(doc, page).unwrap_or_default();
        let blocks = pdfcraft_edit::text_lines(doc, page).map_err(err)?;
        let mut result = Vec::new();
        for (index, b) in blocks.iter().enumerate() {
            if !is_page_number(&b.text) {
                continue;
            }
            let [x0, y0, x1, y1] = layout::enclosing_box(b.rect, &rules);
            let corners = [[x0, y0], [x0, y1], [x1, y0], [x1, y1]]
                .map(|p| info.user_to_view(p[0] as f32, p[1] as f32));
            let x = corners
                .iter()
                .map(|p| p[0] as f64)
                .fold(f64::INFINITY, f64::min);
            let y = corners
                .iter()
                .map(|p| p[1] as f64)
                .fold(f64::INFINITY, f64::min);
            let right = corners
                .iter()
                .map(|p| p[0] as f64)
                .fold(f64::NEG_INFINITY, f64::max);
            let bottom = corners
                .iter()
                .map(|p| p[1] as f64)
                .fold(f64::NEG_INFINITY, f64::max);
            // Keep exact detected bounds: never erase neighbouring borders or dimensions.
            result.push(Candidate {
                page,
                area: Area {
                    x,
                    y,
                    width: right - x,
                    height: bottom - y,
                },
                text: b.text.clone(),
                text_rect: b.rect,
                line: Some(index),
                source_size: Some(b.size),
            });
        }
        Ok(result)
    }
    pub fn marker_candidates(
        &self,
        page: usize,
        pattern: &marker::Pattern,
    ) -> Result<Vec<Candidate>> {
        let image = self.render(page, 2.0)?;
        Ok(marker::find_pattern(&image, pattern)
            .into_iter()
            .map(|area| Candidate {
                page,
                area: Area {
                    x: area.x / 2.0,
                    y: area.y / 2.0,
                    width: area.width / 2.0,
                    height: area.height / 2.0,
                },
                text: "Marcador (revisar)".into(),
                text_rect: [0.0; 4],
                line: None,
                source_size: None,
            })
            .collect())
    }
    pub fn numbered(&self, config: &Numbering) -> Result<Self> {
        if config.first_page > config.last_page
            || config.last_page >= self.pages.len()
            || !config.size.is_finite()
            || !(1.0..=500.0).contains(&config.size)
            || config.prefix.len() > 120
            || config.separator.len() > 20
        {
            return Err(err("Configuração de numeração inválida"));
        }
        let count = config.last_page - config.first_page + 1;
        let mut doc = self.document()?;
        for (offset, page) in (config.first_page..=config.last_page).enumerate() {
            let info = &self.pages[page];
            let mut preferred_size = config.size;
            let area = match config.mode {
                Mode::Manual => {
                    if self
                        .candidates_in(&doc, page)?
                        .iter()
                        .any(|c| c.area.overlaps(config.area))
                    {
                        return Err(err(format!(
                            "Página {}: já existe contagem nessa região. Use Substituir contagem existente e, se necessário, selecione a região de busca.",
                            page + 1
                        )));
                    }
                    config.area
                }
                mode => {
                    let mut candidates = if mode == Mode::Existing {
                        self.candidates_in(&doc, page)?
                    } else {
                        self.marker_candidates(page, &config.marker_pattern)?
                    };
                    if mode == Mode::Existing && config.restrict_area {
                        candidates.retain(|c| c.area.overlaps(config.area));
                    }
                    if candidates.is_empty()
                        || (candidates.len() != 1
                            && (mode != Mode::Existing
                                || !candidates.iter().all(|c| {
                                    same_number_position(c.text_rect, candidates[0].text_rect)
                                })))
                    {
                        return Err(err(format!(
                            "Página {}: {} candidatos. Selecione a região de busca em Substituir contagem existente ou ajuste o intervalo.",
                            page + 1,
                            candidates.len()
                        )));
                    }
                    let c = &candidates[0];
                    if config.inherit_size {
                        preferred_size = c.source_size.unwrap_or(config.size);
                    }
                    if c.line.is_some() {
                        // Refresh indices after every removal; copied/overlaid counts can have
                        // distinct operators and must all disappear before adding new text.
                        for attempt in 0..=32 {
                            let remaining = self.candidates_in(&doc, page)?;
                            let old = remaining
                                .iter()
                                .find(|old| same_number_position(old.text_rect, c.text_rect));
                            let Some(old) = old else {
                                break;
                            };
                            if attempt == 32 {
                                return Err(err(format!(
                                    "Página {}: não foi possível remover toda a contagem anterior",
                                    page + 1
                                )));
                            }
                            pdfcraft_edit::replace_line(
                                &mut doc,
                                page,
                                old.line.ok_or_else(|| err("Contagem não editável"))?,
                                "",
                            )
                            .map_err(err)?;
                        }
                    }
                    if mode == Mode::PurpleMarker {
                        if !config.allow_marker_cover {
                            return Err(err(
                                "Ative a cobertura branca do marcador após revisar a prévia. O marcador original permanece sob a cobertura.",
                            ));
                        }
                        // stamp uses user-space coordinates; map all corners for page rotations.
                        let a = c.area;
                        let points = [
                            [a.x, a.y],
                            [a.x + a.width, a.y],
                            [a.x + a.width, a.y + a.height],
                            [a.x, a.y + a.height],
                        ]
                        .map(|p| info.view_to_user(p[0] as f32, p[1] as f32));
                        let stream = format!(
                            "q 1 1 1 rg {} {} m {} {} l {} {} l {} {} l h f Q\n",
                            points[0][0],
                            points[0][1],
                            points[1][0],
                            points[1][1],
                            points[2][0],
                            points[2][1],
                            points[3][0],
                            points[3][1]
                        );
                        pdfcraft_edit::stamp(
                            &mut doc,
                            page,
                            "NumeroPdfMarker",
                            stream.into_bytes(),
                        )
                        .map_err(err)?;
                    }
                    c.area
                }
            };
            if !area.valid(info) {
                return Err(err(format!("Página {}: área fora da folha", page + 1)));
            }
            let text = config.label(offset, count)?;
            let family = match config.font {
                0 => Family::Helvetica,
                1 => Family::Times,
                2 => Family::Courier,
                _ => return Err(err("Fonte inválida")),
            };
            let unit = family.width(&text, 1.0, config.bold, config.italic);
            if unit <= 0.0 {
                return Err(err("Texto sem largura"));
            }
            let fit = (area.width / unit).min(area.height / 1.2);
            let size = if config.auto_fit {
                preferred_size.min(fit)
            } else {
                preferred_size
            };
            if size < 1.0 || size > fit + 0.01 {
                return Err(err(
                    "Texto não cabe na área; diminua a fonte ou ative ajuste automático",
                ));
            }
            let top = info.height as f64 - area.y - (area.height - size * 1.2) / 2.0;
            let content = Content::Text(AddedText {
                rect: [area.x, top - size * 1.2, area.x + area.width, top],
                text,
                family,
                bold: config.bold,
                italic: config.italic,
                size,
                color: config.color.map(|v| v as f64 / 255.0),
                ink: None,
                align: Align::Center,
                char_spacing: 0.0,
                aligns: vec![],
            });
            pdfcraft_edit::add_content(&mut doc, page, &content).map_err(err)?;
        }
        Self::from_bytes(write_full(&doc, &SaveOptions::default()).map_err(err)?)
    }
    pub fn extract(&self, indices: &[usize]) -> Result<Self> {
        if indices.is_empty() || indices.iter().any(|&p| p >= self.pages.len()) {
            return Err(err("Seleção de páginas inválida"));
        }
        let src = self.document()?;
        let mut dst = Document::new_empty();
        pdfcraft_organize::import_pages(&mut dst, &src, indices, 0).map_err(err)?;
        Self::from_bytes(write_full(&dst, &SaveOptions::default()).map_err(err)?)
    }
    pub fn merge(&self, other: &Self) -> Result<Self> {
        if self.pages.len() + other.pages.len() > 10_000 {
            return Err(err("Limite: 10.000 páginas"));
        }
        let mut dst = self.document()?;
        let src = other.document()?;
        pdfcraft_organize::import_pages(
            &mut dst,
            &src,
            &(0..other.pages.len()).collect::<Vec<_>>(),
            self.pages.len(),
        )
        .map_err(err)?;
        Self::from_bytes(write_full(&dst, &SaveOptions::default()).map_err(err)?)
    }
    /// A new path only. Atomic no-clobber write keeps originals safe even on concurrent saves.
    pub fn save(&self, path: &Path) -> Result<()> {
        save_new(path, self.bytes.as_slice())
    }
}
pub fn save_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(err)?;
    tmp.write_all(bytes).map_err(err)?;
    tmp.as_file().sync_all().map_err(err)?;
    tmp.persist_noclobber(path).map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sequence_offset_and_total_are_independent() {
        let c = Numbering {
            start: 15,
            ..Default::default()
        };
        assert_eq!(c.label(0, 6).unwrap(), "15/06");
        assert_eq!(
            Numbering {
                total_is_last: true,
                ..c
            }
            .label(5, 6)
            .unwrap(),
            "20/20"
        );
    }
    #[test]
    fn rejects_overflow() {
        let c = Numbering {
            start: u32::MAX,
            ..Default::default()
        };
        assert!(c.label(1, 2).is_err());
    }
    #[test]
    fn detects_only_complete_counts() {
        for s in ["01/06", "1/9"] {
            assert!(is_page_number(s));
        }
        for s in ["VISTA 01/06", "01/06/26", "/6", "1/a", "123"] {
            assert!(!is_page_number(s));
        }
    }
    #[test]
    fn save_never_replaces_original() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.pdf");
        save_new(&p, b"first").unwrap();
        assert!(save_new(&p, b"second").is_err());
        assert_eq!(std::fs::read(p).unwrap(), b"first");
    }
}
