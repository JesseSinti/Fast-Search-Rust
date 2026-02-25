use eframe::egui;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;


use fast_search::{run_search, SearchOptions, SearchResult}; 

fn main() -> eframe::Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions{
        viewport: egui::ViewportBuilder::default().with_transparent(true),
        ..Default::default()
    };
    
    eframe::run_native(
        "Fast Search Engine",
        native_options,
        Box::new(|_cc| Ok(Box::new(FastSearchApp::default()))),
    )
}

struct FastSearchApp {
   
    root_path: String,
    search_term: String,
    file_name: String,
    ignore_case: bool,
    max_depth: usize,
    file_types: Option<String>,
    file_scanned: usize,
    has_searched: bool,
    results: Vec<SearchResult>,
    is_searching: bool,
    cancel_token: Arc<AtomicBool>,
    
   
    receiver: Option<Receiver<SearchResult>>,
}



impl Default for FastSearchApp {
    fn default() -> Self {
        let cancel_token = Arc::new(AtomicBool::new(true));
        cancel_token.store(false, Ordering::SeqCst);
        Self {
            root_path: std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| ".".to_string()),
            search_term: "".to_string(),
            file_name: "".to_string(),
            ignore_case: false,
            max_depth: 255,
            file_types: Option::default(),
            file_scanned: 0,
            results: Vec::new(),
            is_searching: false,
            has_searched: false,
            receiver: None,
            cancel_token: cancel_token,
            
            
        }
    }
}

impl eframe::App for FastSearchApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Setup Visuals
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(30, 30, 30);
        ctx.set_visuals(visuals);

        if let Some(ref rx) = self.receiver {
            loop {
                match rx.try_recv() {
                    Ok(result) => match result {
                        SearchResult::FileNameMatch { .. } | SearchResult::ContentMatch { .. } => {
                            self.results.push(result);
                        }
                        SearchResult::ProgressUpdate(count) => {
                            self.file_scanned += count;
                        }
                    },
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.is_searching = false;
                        self.receiver = None;
                        break;
                    }
                }
            }
            if self.is_searching {
                ctx.request_repaint();
            }
        }

        egui::SidePanel::left("SearchChoices")
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.heading("🔍 FastSearch");
                ui.separator();

                let mut submit_request = false;
                let input_width = ui.available_width() - 35.0;

                ui.label("Root Path:");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.root_path).desired_width(input_width));
                    
                });

                if ui.button("📁").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.root_path = path.display().to_string();
                        }
                    }

                ui.label("Search Text:");
                let res1 = ui.add(egui::TextEdit::singleline(&mut self.search_term).desired_width(input_width));
                
                ui.label("Search File Name:");
                let res2 = ui.add(egui::TextEdit::singleline(&mut self.file_name).desired_width(input_width));

                ui.label(egui::RichText::new("File Types/Extensions").color(egui::Color32::WHITE).strong());

                if self.file_types.is_none() {

                    self.file_types = Some(String::new());

                }

                ui.add(egui::TextEdit::singleline( self.file_types.as_mut().unwrap())

                .desired_width(input_width)

                .hint_text("File Types")

                .text_color(egui::Color32::LIGHT_GRAY));

                ui.end_row();

                if (res1.lost_focus() || res2.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit_request = true;
                }

                ui.collapsing("Advanced Options", |ui| {
                    ui.checkbox(&mut self.ignore_case, "Ignore Case");
                    
                    let mut depth = self.max_depth as u32;
                    if ui.add(egui::DragValue::new(&mut depth).range(0..=5000)).changed() {
                        self.max_depth = depth as usize;
                    }
                });

                ui.add_space(20.0);

                ui.horizontal(|ui| {
                    if self.is_searching {
                        let cancel_btn = egui::Button::new(egui::RichText::new("🛑 Cancel").color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(200, 40, 40));
                        if ui.add(cancel_btn).clicked() {
                            self.cancel_token.store(true, std::sync::atomic::Ordering::Relaxed);
                            self.is_searching = false;
                        }
                    } else {
                        if ui.button("🚀 Start Search").clicked() || submit_request {
                            self.execute_search(ctx.clone());
                        }
                    }
                }); 
            });

        if self.is_searching {
            egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.label(format!("Scanning... ({} files)", self.file_scanned));
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.results.is_empty() {
                ui.centered_and_justified(|ui| {
                    if self.has_searched && !self.is_searching {
                        ui.label(egui::RichText::new("No matches found.").color(egui::Color32::LIGHT_RED));
                    } else {
                        ui.label("Enter parameters to begin.");
                    }
                });
            } else {
                let row_height = ui.text_style_height(&egui::TextStyle::Body);
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show_rows(ui, row_height, self.results.len(), |ui, row_range| {
                        for i in row_range {
                            if let Some(res) = self.results.get(i) {
                                self.render_result_row(ui, res);
                            }
                        }
                    });
            }
        });
    }

} 



impl FastSearchApp {
    fn execute_search(&mut self, ctx: egui::Context) {

        self.cancel_token.store(true, Ordering::Relaxed);

        self.cancel_token = Arc::new(AtomicBool::new(false));
        let thread_token = Arc::clone(&self.cancel_token);

        self.has_searched = true;
        if self.search_term.is_empty() && self.file_name.is_empty() { return; }

        if self.root_path.ends_with(":") {
            self.root_path.push_str("\\");
        };

        
        self.results.clear();
        self.is_searching = true;
        
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);

        let cleaned_file_types = self.file_types.as_ref()
        .filter(|s| !s.trim().is_empty())
        .cloned();

        let options = SearchOptions {
            root: self.root_path.clone(),
            text_query: if self.search_term.trim().is_empty() { None } else { Some(self.search_term.clone()) },
            file_query: if self.file_name.trim().is_empty() { None } else { Some(self.file_name.clone())},
            ignore_case: self.ignore_case.clone(),
            max_depth: self.max_depth.clone(),
            file_types: cleaned_file_types,
        };


        thread::spawn(move || {
            run_search(options, tx, thread_token);
            ctx.request_repaint(); 
        });
    }

    fn render_result_row(&self, ui: &mut egui::Ui, result: &SearchResult) {
    match result {
        SearchResult::FileNameMatch { path } => {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("FILE")
                        .color(egui::Color32::from_rgb(0, 255, 127))
                        .strong(),
                );

                let response = ui.add(
                    egui::Label::new(
                        egui::RichText::new(path.to_string_lossy())
                            .color(egui::Color32::WHITE),
                    )
                    .wrap(),
                );

                if response.clicked() {
                    let _ = open::that(path);
                }
                if response.secondary_clicked() {
                    let _ = open::that(path.parent().unwrap_or(path));
                }
            });

            ui.separator();
        }

        SearchResult::ContentMatch { path, line_number, line_text } => {
            ui.vertical(|ui| {
                let response = ui.add(
                    egui::Label::new(
                        egui::RichText::new(path.to_string_lossy())
                            .color(egui::Color32::LIGHT_GRAY),
                    )
                    .wrap(), 
                );

                if response.clicked() {
                    let _ = open::that(path);
                }
                if response.secondary_clicked() {
                    let _ = open::that(path.parent().unwrap_or(path));
                }

                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!(
                            "{}: {}",
                            line_number,
                            line_text.trim()
                        ))
                        .color(egui::Color32::WHITE),
                    )
                    .wrap(),
                );
            });

            ui.separator();
        }

        SearchResult::ProgressUpdate(_) => {}
    }
}
}