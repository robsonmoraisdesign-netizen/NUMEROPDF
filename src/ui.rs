use eframe::egui::{self, Color32, RichText};
use numeropdf::{Area, Mode, Numbering, Pdf};
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};

enum Done {
    Document(Arc<Pdf>, String),
    Preview(Arc<Pdf>),
    Batch(Vec<numeropdf::batch::Outcome>),
    Message(String),
}
type Work = std::result::Result<Done, String>;
pub struct App {
    document: Option<Arc<Pdf>>,
    preview: Option<Arc<Pdf>>,
    undo: Option<Arc<Pdf>>,
    config: Numbering,
    page: usize,
    zoom: f32,
    dpi: u32,
    viewer: crate::viewer::Viewer,
    files: Vec<std::path::PathBuf>,
    pending_open: Option<std::path::PathBuf>,
    batch_to_end: bool,
    results: Vec<numeropdf::batch::Outcome>,
    progress: Arc<std::sync::atomic::AtomicUsize>,
    fit_page: bool,
    previous_view: Option<(usize, u32)>,
    #[cfg(test)]
    geometry: Option<(egui::Rect, egui::Rect)>,
    rx: Option<mpsc::Receiver<Work>>,
    status: String,
    name: String,
    show_preview: bool,
    choose_area: bool,
    drag_start: Option<egui::Pos2>,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self::initial()
    }
    fn initial() -> Self {
        Self {
            document: None,
            preview: None,
            undo: None,
            config: Numbering::default(),
            page: 0,
            zoom: 1.0,
            dpi: 150,
            viewer: Default::default(),
            files: Vec::new(),
            pending_open: None,
            batch_to_end: true,
            results: Vec::new(),
            progress: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            fit_page: true,
            previous_view: None,
            #[cfg(test)]
            geometry: None,
            rx: None,
            status: "Abra um PDF para começar".into(),
            name: String::new(),
            show_preview: true,
            choose_area: false,
            drag_start: None,
        }
    }
    fn launch(&mut self, label: &str, work: impl FnOnce() -> Work + Send + 'static) {
        if self.rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.status = label.into();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| {
                    Err("Falha ao processar o PDF; o original foi preservado".into())
                });
            let _ = tx.send(result);
        });
    }
    fn visible(&self) -> Option<Arc<Pdf>> {
        if self.show_preview {
            self.preview.clone().or_else(|| self.document.clone())
        } else {
            self.document.clone()
        }
    }
    fn changed(&mut self) {
        self.preview = None;
    }
    fn poll(&mut self, ctx: &egui::Context) {
        let result = self.rx.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("Processamento interrompido".into())),
            Err(mpsc::TryRecvError::Empty) => None,
        });
        if let Some(result) = result {
            self.rx = None;
            match result {
                Ok(Done::Document(doc, name)) => {
                    self.undo = self.document.take();
                    self.config.last_page = doc.pages.len() - 1;
                    self.config.first_page = usize::from(doc.pages.len() > 1);
                    self.document = Some(doc);
                    self.name = name;
                    self.page = 0;
                    self.preview = None;
                    self.fit_page = true;

                    self.status =
                        "PDF aberto. Configure a numeração ou organize as páginas.".into();
                }
                Ok(Done::Preview(pdf)) => {
                    self.preview = Some(pdf);
                    self.show_preview = true;

                    self.status = "Prévia pronta. Confira as páginas e salve uma cópia.".into();
                }
                Ok(Done::Batch(results)) => {
                    let ok = results.iter().filter(|r| r.output.is_some()).count();
                    self.status = format!("Lote concluído: {ok}/{} arquivos salvos", results.len());
                    self.results = results;
                }
                Ok(Done::Message(message)) => self.status = message,
                Err(e) => {
                    self.status = format!("Erro: {e}");
                }
            }
        }
        if self.rx.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
    fn add_files(&mut self, paths: Vec<std::path::PathBuf>) {
        let mut first = None;
        for path in paths {
            if !path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("pdf"))
            {
                continue;
            }
            if first.is_none() {
                first = Some(path.clone());
            }
            if !self.files.contains(&path) && self.files.len() < 1000 {
                self.files.push(path);
            }
        }
        if self.document.is_none() && first.is_some() {
            self.pending_open = first;
        }
    }
    fn open_path(&mut self, path: std::path::PathBuf) {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.launch("Abrindo PDF…", move || {
            Pdf::open(&path)
                .map(|p| Done::Document(Arc::new(p), name))
                .map_err(|e| e.to_string())
        });
    }
    fn open(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .pick_files()
        {
            self.pending_open = paths.first().cloned();
            self.add_files(paths);
        }
    }
    fn batch_controls(&mut self, ui: &mut egui::Ui) {
        ui.collapsing(format!("Lote · {} PDFs", self.files.len()), |ui| {
            if ui.button("Adicionar PDFs").clicked() {
                self.open();
            }
            let mut remove = None;
            egui::ScrollArea::vertical()
                .id_salt("batch-files")
                .max_height(180.0)
                .show_rows(ui, 24.0, self.files.len(), |ui, range| {
                    for i in range {
                        ui.horizontal(|ui| {
                            if ui.small_button("×").clicked() {
                                remove = Some(i);
                            }
                            if ui
                                .selectable_label(
                                    false,
                                    self.files[i]
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy(),
                                )
                                .clicked()
                            {
                                self.pending_open = Some(self.files[i].clone());
                            }
                        });
                    }
                });
            if let Some(i) = remove {
                self.files.remove(i);
            }
            ui.checkbox(
                &mut self.batch_to_end,
                "Até a última folha de cada documento",
            );
            ui.label("A sequência reinicia em cada PDF. Originais preservados.");
            if ui
                .add_enabled(
                    self.rx.is_none() && !self.files.is_empty(),
                    egui::Button::new("Numerar lote e salvar…"),
                )
                .clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                let inputs = self.files.clone();
                let config = self.config.clone();
                let to_end = self.batch_to_end;
                let progress = self.progress.clone();
                progress.store(0, std::sync::atomic::Ordering::Relaxed);
                self.results.clear();
                self.launch("Numerando lote…", move || {
                    Ok(Done::Batch(numeropdf::batch::number_files(
                        &inputs,
                        &dir,
                        &config,
                        to_end,
                        |done, _| {
                            progress.store(done, std::sync::atomic::Ordering::Relaxed);
                        },
                    )))
                })
            }
            ui.label(format!(
                "Processados: {}",
                self.progress.load(std::sync::atomic::Ordering::Relaxed)
            ));
            for result in &self.results {
                if let Some(e) = &result.error {
                    ui.colored_label(Color32::YELLOW, format!("{}: {e}", result.input.display()));
                }
            }
        });
    }
    fn save(&mut self) {
        if let Some(pdf) = self.visible()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name("numeropdf-exportado.pdf")
                .save_file()
        {
            self.launch("Salvando cópia…", move || {
                pdf.save(&path)
                    .map(|_| Done::Message("Cópia salva. Arquivo original preservado.".into()))
                    .map_err(|e| e.to_string())
            });
        }
    }
    fn merge(&mut self) {
        if let Some(doc) = self.document.clone()
            && let Some(paths) = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .pick_files()
        {
            self.launch("Mesclando PDFs…", move || {
                let mut current =
                    Pdf::from_bytes(doc.bytes.as_ref().clone()).map_err(|e| e.to_string())?;
                for path in paths {
                    let other = Pdf::open(&path).map_err(|e| e.to_string())?;
                    current = current.merge(&other).map_err(|e| e.to_string())?;
                }
                Ok(Done::Document(
                    Arc::new(current),
                    "Documentos mesclados".into(),
                ))
            });
        }
    }
    fn extract(&mut self, one: bool) {
        if let Some(doc) = self.document.clone() {
            let indices = if one {
                vec![self.page]
            } else {
                (self.config.first_page..=self.config.last_page).collect()
            };
            self.launch("Extraindo páginas…", move || {
                doc.extract(&indices)
                    .map(|p| Done::Document(Arc::new(p), "Páginas extraídas".into()))
                    .map_err(|e| e.to_string())
            });
        }
    }
    fn split(&mut self) {
        if let Some(doc) = self.visible()
            && let Some(parent) = rfd::FileDialog::new().pick_folder()
        {
            self.launch("Dividindo PDF…", move || {
                let dir = parent.join("NUMEROPDF-paginas");
                std::fs::create_dir(&dir)
                    .map_err(|e| format!("Escolha pasta sem NUMEROPDF-paginas: {e}"))?;
                for i in 0..doc.pages.len() {
                    doc.extract(&[i])
                        .and_then(|p| p.save(&dir.join(format!("pagina-{:04}.pdf", i + 1))))
                        .map_err(|e| format!("Salvas {i} páginas antes da falha: {e}"))?;
                }
                Ok(Done::Message(format!(
                    "{} PDFs salvos em {}",
                    doc.pages.len(),
                    dir.display()
                )))
            });
        }
    }
    fn png(&mut self) {
        if let Some(doc) = self.visible()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .set_file_name("pagina.png")
                .save_file()
        {
            let page = self.page;
            let dpi = self.dpi;
            self.launch("Exportando PNG…", move || {
                let im = doc
                    .render(page, dpi as f32 / 72.0)
                    .map_err(|e| e.to_string())?;
                let mut bytes = std::io::Cursor::new(Vec::new());
                im.write_to(&mut bytes, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                numeropdf::save_new(&path, bytes.get_ref()).map_err(|e| e.to_string())?;
                Ok(Done::Message("PNG salvo".into()))
            });
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Numeração de pranchas");
        ui.label("Configure, revise e salve uma cópia.");
        ui.separator();
        let old = self.config.clone();
        let len = self.document.as_ref().map_or(1, |p| p.pages.len());
        self.config.first_page = self.config.first_page.min(len - 1);
        self.config.last_page = self.config.last_page.min(len - 1);
        let mut first = self.config.first_page + 1;
        let mut last = self.config.last_page + 1;
        ui.horizontal(|ui| {
            ui.label("Da folha");
            ui.add(egui::DragValue::new(&mut first).range(1..=len));
            ui.label("até");
            ui.add(egui::DragValue::new(&mut last).range(1..=len));
        });
        self.config.first_page = first - 1;
        self.config.last_page = last - 1;
        ui.horizontal(|ui| {
            ui.label("Número inicial");
            ui.add(egui::DragValue::new(&mut self.config.start).range(0..=999_999));
        });
        ui.horizontal(|ui| {
            ui.label("Dígitos");
            ui.add(egui::DragValue::new(&mut self.config.digits).range(1..=9));
        });
        ui.checkbox(&mut self.config.show_total, "Mostrar total");
        ui.checkbox(
            &mut self.config.total_is_last,
            "Total = último número da sequência",
        );
        ui.horizontal(|ui| {
            ui.label("Prefixo");
            ui.text_edit_singleline(&mut self.config.prefix);
        });
        ui.horizontal(|ui| {
            ui.label("Separador");
            ui.text_edit_singleline(&mut self.config.separator);
        });
        let n = last.saturating_sub(first) + 1;
        if let Ok(label) = self.config.label(0, n) {
            ui.label(
                RichText::new(label)
                    .size(26.0)
                    .color(Color32::from_rgb(185, 153, 255)),
            );
        }
        ui.separator();
        ui.strong("Localizar posição");
        ui.radio_value(
            &mut self.config.mode,
            Mode::Existing,
            "Substituir contagem existente",
        );
        ui.radio_value(
            &mut self.config.mode,
            Mode::PurpleMarker,
            "Marcador colorido",
        );
        ui.radio_value(
            &mut self.config.mode,
            Mode::Manual,
            "Área manual (nova contagem)",
        );
        if self.config.mode == Mode::PurpleMarker {
            ui.horizontal(|ui| {
                ui.label("Cor do marcador");
                ui.color_edit_button_srgb(&mut self.config.marker_pattern.color);
            });
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.config.marker_pattern.symbol,
                    numeropdf::marker::Symbol::X,
                    "Quadrado com X",
                );
                ui.selectable_value(
                    &mut self.config.marker_pattern.symbol,
                    numeropdf::marker::Symbol::Plus,
                    "Quadrado com +",
                );
            });
            ui.add(
                egui::Slider::new(&mut self.config.marker_pattern.tolerance, 5..=100)
                    .text("Tolerância de cor"),
            );
            if ui.button("Salvar marcador SVG").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_file_name("marcador-numeropdf.svg")
                    .save_file()
            {
                let svg = numeropdf::marker::svg(&self.config.marker_pattern);
                self.status = numeropdf::save_new(&path, svg.as_bytes())
                    .map(|_| "Marcador SVG salvo (10 mm, escalável)".into())
                    .unwrap_or_else(|e| e.to_string());
            }

            ui.label(
                "O contorno e o símbolo devem ter a cor escolhida. Detecção por imagem; revise cada página.",
            );
            ui.checkbox(
                &mut self.config.allow_marker_cover,
                "Cobrir marcador de branco (permanece por baixo)",
            );
        }
        if self.config.mode == Mode::Manual {
            if ui.button("Desenhar área na página").clicked() {
                self.choose_area = true;
                self.show_preview = false;
            }
            for (label, v) in [
                ("X (pt)", &mut self.config.area.x),
                ("Y (pt)", &mut self.config.area.y),
                ("Largura", &mut self.config.area.width),
                ("Altura", &mut self.config.area.height),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.add(egui::DragValue::new(v).speed(0.5).range(0.0..=10000.0));
                });
            }
            ui.label("Origem no canto superior esquerdo. A área será aplicada ao intervalo.");
        }
        ui.separator();
        ui.strong("Texto");
        egui::ComboBox::from_label("Fonte")
            .selected_text(
                ["Helvetica", "Times", "Courier"]
                    .get(self.config.font)
                    .copied()
                    .unwrap_or("Helvetica"),
            )
            .show_ui(ui, |ui| {
                for (i, name) in ["Helvetica", "Times", "Courier"].iter().enumerate() {
                    ui.selectable_value(&mut self.config.font, i, *name);
                }
            });
        ui.horizontal(|ui| {
            ui.label("Tamanho");
            ui.add(egui::DragValue::new(&mut self.config.size).range(1.0..=200.0));
            ui.label("pt");
        });
        ui.checkbox(
            &mut self.config.inherit_size,
            "Detectar tamanho da fonte existente",
        );
        ui.checkbox(&mut self.config.auto_fit, "Ajustar ao espaço disponível");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.config.bold, "Negrito");
            ui.checkbox(&mut self.config.italic, "Itálico");
        });
        ui.color_edit_button_srgb(&mut self.config.color);
        if old != self.config {
            self.changed();
        }
        ui.separator();
        if ui
            .add_sized(
                [ui.available_width(), 36.0],
                egui::Button::new("Gerar prévia real"),
            )
            .clicked()
            && let Some(doc) = self.document.clone()
        {
            let c = self.config.clone();
            self.launch("Calculando numeração…", move || {
                doc.numbered(&c)
                    .map(|p| Done::Preview(Arc::new(p)))
                    .map_err(|e| e.to_string())
            });
        }
        ui.checkbox(&mut self.show_preview, "Mostrar resultado (antes/depois)");
        ui.label(if self.preview.is_some() {
            "Prévia calculada e pronta para salvar"
        } else {
            "Sem prévia de numeração calculada"
        });
        ui.horizontal(|ui| {
            if ui.button("Salvar padrão").clicked()
                && let Some(p) = rfd::FileDialog::new()
                    .set_file_name("numeracao.json")
                    .save_file()
            {
                match serde_json::to_vec_pretty(&self.config) {
                    Ok(bytes) => {
                        self.status = numeropdf::save_new(&p, &bytes)
                            .map(|_| "Padrão salvo".into())
                            .unwrap_or_else(|e| e.to_string())
                    }
                    Err(e) => self.status = e.to_string(),
                }
            }
            if ui.button("Carregar padrão").clicked()
                && let Some(p) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
            {
                let result = std::fs::read(p).map_err(|e| e.to_string()).and_then(|b| {
                    serde_json::from_slice::<Numbering>(&b).map_err(|e| e.to_string())
                });
                match result {
                    Ok(c) => {
                        self.config = c;
                        self.changed();
                    }
                    Err(e) => self.status = e,
                }
            }
        });
    }
}
impl App {
    fn draw(&mut self, ctx: &egui::Context) {
        self.poll(ctx);
        let dropped = ctx.input_mut(|i| {
            std::mem::take(&mut i.raw.dropped_files)
                .into_iter()
                .filter_map(|f| f.path)
                .collect()
        });
        self.add_files(dropped);
        if self.rx.is_none()
            && let Some(path) = self.pending_open.take()
        {
            self.open_path(path);
        }

        let busy = self.rx.is_some();
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("NUMERO").strong().size(23.0));
                ui.label(
                    RichText::new("PDF")
                        .strong()
                        .size(23.0)
                        .color(Color32::from_rgb(174, 137, 255)),
                );
                ui.separator();
                ui.add_enabled_ui(!busy, |ui| {
                    if ui.button("Abrir PDF").clicked() {
                        self.open();
                    }
                    ui.add_enabled_ui(self.document.is_some(), |ui| {
                        if ui.button("Mesclar").clicked() {
                            self.merge();
                        }
                        if ui.button("Extrair intervalo").clicked() {
                            self.extract(false);
                        }
                        if ui.button("Dividir").clicked() {
                            self.split();
                        }
                        if ui.button("Salvar cópia").clicked() {
                            self.save();
                        }
                        if ui.button("PNG da página").clicked() {
                            self.png();
                        }
                        ui.add(
                            egui::DragValue::new(&mut self.dpi)
                                .range(72..=600)
                                .suffix(" dpi"),
                        );
                    });
                    if ui
                        .add_enabled(
                            self.undo.is_some(),
                            egui::Button::new("Desfazer organização"),
                        )
                        .clicked()
                        && let Some(old) = self.undo.take()
                    {
                        self.document = Some(old);
                        self.page = 0;
                        self.changed();
                        self.config.last_page =
                            self.document.as_ref().map_or(0, |p| p.pages.len() - 1);
                    }
                });
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.rx.is_some() {
                    ui.spinner();
                }
                ui.label(&self.status);
            });
        });
        egui::SidePanel::right("settings")
            .default_width(310.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.batch_controls(ui);
                    ui.add_enabled_ui(!busy && self.document.is_some(), |ui| self.settings(ui));
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(pdf) = self.visible() {
                ui.horizontal(|ui| {
                    ui.strong(&self.name);
                    ui.separator();
                    let mut number = self.page + 1;
                    if ui
                        .add_enabled(self.page > 0, egui::Button::new("◀"))
                        .clicked()
                    {
                        number -= 1;
                    }
                    ui.add_enabled(
                        true,
                        egui::DragValue::new(&mut number)
                            .range(1..=pdf.pages.len())
                            .prefix("Página "),
                    );
                    if ui
                        .add_enabled(number < pdf.pages.len(), egui::Button::new("▶"))
                        .clicked()
                    {
                        number += 1;
                    }
                    self.page = (number - 1).min(pdf.pages.len() - 1);
                    ui.label(format!("/ {}", pdf.pages.len()));
                    ui.add_enabled(
                        true,
                        egui::Slider::new(&mut self.zoom, 0.1..=8.0)
                            .text("Zoom")
                            .fixed_decimals(2),
                    );
                });
                let viewport = ui.available_rect_before_wrap();
                let info = &pdf.pages[self.page];
                if self.fit_page {
                    self.zoom = ((viewport.width() - 24.0) / info.width)
                        .min((viewport.height() - 24.0) / info.height)
                        .clamp(0.1, 8.0);
                    self.fit_page = false;
                }
                if ctx.input(|i| {
                    i.modifiers.ctrl && i.pointer.hover_pos().is_some_and(|p| viewport.contains(p))
                }) {
                    let delta = ctx.input_mut(|i| {
                        let d = i.raw_scroll_delta.y;
                        i.raw_scroll_delta = egui::Vec2::ZERO;
                        i.smooth_scroll_delta = egui::Vec2::ZERO;
                        d
                    });
                    self.zoom = numeropdf::layout::wheel_zoom(self.zoom, delta);
                }
                if let Some(e) =
                    self.viewer
                        .update(ctx, &pdf, self.page, self.zoom * ctx.pixels_per_point())
                {
                    self.status = e;
                }
                if self.choose_area {
                    ui.colored_label(
                        Color32::YELLOW,
                        "Arraste sobre a página para definir a área de numeração",
                    );
                }
                let view = (self.page, (self.zoom * 10000.0) as u32);
                let recenter = self.previous_view != Some(view);
                self.previous_view = Some(view);
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(texture) = self.viewer.texture() {
                            let size = egui::vec2(info.width, info.height) * self.zoom;
                            let canvas = size.max(viewport.size());
                            let (rect, _) = ui.allocate_exact_size(canvas, egui::Sense::hover());
                            let offset = egui::vec2(
                                numeropdf::layout::centered_offset(canvas.x, size.x),
                                numeropdf::layout::centered_offset(canvas.y, size.y),
                            );
                            let page_rect = egui::Rect::from_min_size(rect.min + offset, size);
                            #[cfg(test)]
                            {
                                self.geometry = Some((viewport, page_rect));
                            }
                            if recenter {
                                ui.scroll_to_rect(
                                    egui::Rect::from_center_size(
                                        page_rect.center(),
                                        egui::vec2(1.0, 1.0),
                                    ),
                                    Some(egui::Align::Center),
                                );
                            }
                            let response = ui.put(
                                page_rect,
                                egui::Image::new((texture.id(), size)).sense(egui::Sense::drag()),
                            );
                            if self.choose_area {
                                if response.drag_started() {
                                    self.drag_start = response.interact_pointer_pos();
                                }
                                if let (Some(start), Some(end)) =
                                    (self.drag_start, response.interact_pointer_pos())
                                {
                                    let selected = egui::Rect::from_two_pos(start, end)
                                        .intersect(response.rect);
                                    ui.painter().rect_stroke(
                                        selected,
                                        0.0,
                                        egui::Stroke::new(
                                            2.0_f32,
                                            Color32::from_rgb(180, 100, 255),
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                    if response.drag_stopped() {
                                        let min = (selected.min - response.rect.min) / self.zoom;
                                        let area = selected.size() / self.zoom;
                                        if area.x > 2.0 && area.y > 2.0 {
                                            self.config.area = Area {
                                                x: min.x as f64,
                                                y: min.y as f64,
                                                width: area.x as f64,
                                                height: area.y as f64,
                                            };
                                            self.changed();
                                        }
                                        self.choose_area = false;
                                        self.drag_start = None;
                                    }
                                }
                            }
                        } else {
                            ui.label("Aguardando renderização…");
                        }
                    });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.heading("Suas pranchas, na sequência certa.");
                    ui.label("Abra um PDF para visualizar, organizar ou numerar.");
                    ui.add_space(20.0);
                    if ui.button("Escolher PDF").clicked() && !busy {
                        self.open();
                    }
                });
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn centered_page_and_ctrl_wheel_zoom() {
        let mut doc = pdfcraft_cos::Document::new_empty();
        pdfcraft_organize::insert_blank_page(&mut doc, 0, 500.0, 400.0).unwrap();
        let pdf =
            Pdf::from_bytes(pdfcraft_cos::write_full(&doc, &Default::default()).unwrap()).unwrap();
        let mut app = App::initial();
        app.document = Some(Arc::new(pdf));
        let ctx = egui::Context::default();
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1320.0, 850.0),
            )),
            ..Default::default()
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while app.geometry.is_none() {
            let _ = ctx.run(input(), |ctx| app.draw(ctx));
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        let (viewport, page) = app.geometry.unwrap();
        assert!((viewport.center() - page.center()).length() < 2.0);
        let before = app.zoom;
        let ctrl = egui::Modifiers {
            ctrl: true,
            ..Default::default()
        };
        let mut raw = input();
        raw.modifiers = ctrl;
        raw.events = vec![
            egui::Event::PointerMoved(viewport.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 100.0),
                modifiers: ctrl,
            },
        ];
        let _ = ctx.run(raw, |ctx| app.draw(ctx));
        assert!(app.zoom > before);
        assert_eq!(ctx.zoom_factor(), 1.0);
    }
    #[test]
    fn dropped_batch_is_deduplicated_and_queued_while_busy() {
        let mut app = App::initial();
        let (_tx, rx) = mpsc::channel();
        app.rx = Some(rx);
        app.add_files(vec![
            "a.pdf".into(),
            "b.PDF".into(),
            "bad.txt".into(),
            "a.pdf".into(),
        ]);
        assert_eq!(app.files.len(), 2);
        assert_eq!(app.pending_open, Some("a.pdf".into()));
        app.add_files(vec![]);
        assert_eq!(app.pending_open, Some("a.pdf".into()));
    }
}
