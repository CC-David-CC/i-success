//! The I-SUCCESS window, drawn with egui.
//!
//! [`App::show`] draws one frame: the brand bar at the top, then one of three screens. Home has
//! the subject pills, the level bands that collapse and expand, and a card per grade. A grade
//! lists its skills. Practice shows one problem at a time with answer boxes, Check, worked
//! steps and a score.
//!
//! Nothing below this crate knows about egui; the screens read a [`Catalog`] and drive a
//! [`Practice`] from `i-success-engine`.

pub mod theme;

use egui::{
    Align, Align2, Button, CollapsingHeader, FontId, Frame, Layout, Margin, RichText, Role, ScrollArea, Sense, Stroke, TextEdit, Ui, WidgetInfo, vec2,
};
use i_success_engine::{Practice, Verdict};
use i_success_model::{Catalog, Grade, Skill, Subject};
use std::collections::BTreeSet;
use theme::*;

/// The orangutan, 256 x 256: the window icon and the mark on the brand bar.
#[cfg(not(target_arch = "wasm32"))]
pub const ORANGUTAN_PNG: &[u8] = include_bytes!("../../../assets/brand/orangutan-256.png");
/// "I-SUCCESS" on a white tile, 430 x 96.
#[cfg(not(target_arch = "wasm32"))]
pub const WORDMARK_PNG: &[u8] = include_bytes!("../../../assets/brand/wordmark.png");
#[cfg(not(target_arch = "wasm32"))]
const WORDMARK_ASPECT: f32 = 430.0 / 96.0;

/// The bands open when the app starts.
const OPEN_AT_START: [&str; 2] = ["early", "elementary"];
/// Cards per row, at most; fewer when the window is narrow.
const MAX_COLUMNS: usize = 3;
const CARD_MIN_WIDTH: f32 = 300.0;

/// Which screen is showing.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "view", content = "selection")]
pub enum Screen {
    Home,
    /// The skills of one grade, by grade id.
    Grade(String),
    /// Practising one skill.
    Practice(Practice),
}

/// An editable practice document contains no identity or browser session data.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PracticeDocument {
    format: String,
    subject: String,
    bands: BTreeSet<String>,
    screen: Screen,
}

/// A move between screens, decided while drawing and applied after it.
enum Nav {
    Home,
    Grade(String),
    Practice(String),
}

/// The whole app: the catalog, the chosen subject, which bands are open, and the screen.
pub struct App {
    catalog: Result<Catalog, String>,
    subject: String,
    open_bands: BTreeSet<String>,
    screen: Screen,
    styled: bool,
}

impl App {
    /// The app over the built-in catalog. If the catalog cannot be read, the window says why
    /// instead of starting empty.
    pub fn new() -> Self {
        Self::with_catalog(i_success_content::builtin().map_err(|e| e.to_string()))
    }

    /// The app over a given catalog, or the reason there is none.
    pub fn with_catalog(catalog: Result<Catalog, String>) -> Self {
        let subject = catalog.as_ref().ok().and_then(|c| c.subjects.iter().find(|s| s.ready)).map(|s| s.id.clone()).unwrap_or_default();
        Self { catalog, subject, open_bands: OPEN_AT_START.iter().map(|s| (*s).to_owned()).collect(), screen: Screen::Home, styled: false }
    }

    /// Save curriculum navigation, typed answers, hints and score.
    pub fn document(&self) -> Result<String, String> {
        let doc = PracticeDocument {
            format: "i-success/1".into(),
            subject: self.subject.clone(),
            bands: self.open_bands.clone(),
            screen: self.screen.clone(),
        };
        toml::to_string(&doc).map_err(|e| e.to_string())
    }

    /// Only install a complete supported session that references the current catalog.
    pub fn load_document(&mut self, content: &str) -> Result<(), String> {
        if content.len() > 128 * 1024 {
            return Err("Practice file exceeds 128 KiB.".into());
        }
        let doc: PracticeDocument = toml::from_str(content).map_err(|e| e.message().to_owned())?;
        let catalog = self.catalog.as_ref().map_err(Clone::clone)?;
        if doc.format != "i-success/1"
            || !catalog.subject(&doc.subject).is_some_and(|s| s.ready)
            || doc.bands.iter().any(|id| !catalog.bands.iter().any(|b| &b.id == id))
        {
            return Err("Unsupported practice file or unknown curriculum.".into());
        }
        let valid = match &doc.screen {
            Screen::Home => true,
            Screen::Grade(id) => catalog.grade(id).is_some(),
            Screen::Practice(session) => {
                catalog.skill(session.skill_id()).is_some_and(|skill| skill.subject == doc.subject && session.matches_skill(skill))
            }
        };
        if !valid {
            return Err("Practice file refers to an unavailable problem.".into());
        }
        self.subject = doc.subject;
        self.open_bands = doc.bands;
        self.screen = doc.screen;
        Ok(())
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// Opens a screen directly, as a link would.
    pub fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
    }

    /// Whether a band is expanded.
    pub fn is_open(&self, band: &str) -> bool {
        self.open_bands.contains(band)
    }

    /// Draws one frame.
    pub fn show(&mut self, ui: &mut Ui) {
        if !self.styled {
            egui_extras::install_image_loaders(ui.ctx());
            ui.ctx().set_theme(egui::Theme::Light);
            let mut visuals = egui::Visuals::light();
            visuals.panel_fill = PAGE;
            visuals.override_text_color = Some(TEXT);
            visuals.hyperlink_color = LINK;
            ui.ctx().set_visuals(visuals);
            // Larger than egui's defaults: young learners read this.
            ui.ctx().all_styles_mut(|style| {
                style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(16.0));
                style.text_styles.insert(egui::TextStyle::Button, FontId::proportional(16.0));
                style.text_styles.insert(egui::TextStyle::Small, FontId::proportional(13.0));
                style.text_styles.insert(egui::TextStyle::Heading, FontId::proportional(26.0));
            });
            self.styled = true;
        }

        let mut nav = None;
        egui::Panel::top("brand-bar").frame(Frame::new().fill(BRAND).inner_margin(Margin::symmetric(20, 10))).show_separator_line(false).show(
            ui,
            |ui| {
                ui.horizontal(|ui| {
                    #[cfg(not(target_arch = "wasm32"))]
                    let wordmark = {
                        ui.add(
                            egui::Image::from_bytes("bytes://i-success/orangutan.png", ORANGUTAN_PNG)
                                .fit_to_exact_size(vec2(44.0, 44.0))
                                .corner_radius(8.0)
                                .alt_text("Orangutan"),
                        );
                        ui.add_space(6.0);
                        ui.add(
                            egui::Image::from_bytes("bytes://i-success/wordmark.png", WORDMARK_PNG)
                                .fit_to_exact_size(vec2(36.0 * WORDMARK_ASPECT, 36.0))
                                .sense(Sense::click())
                                .alt_text("I-SUCCESS"),
                        )
                    };
                    #[cfg(target_arch = "wasm32")]
                    let wordmark = ui.link(RichText::new("I-SUCCESS").size(24.0).strong().color(TEXT));
                    wordmark.widget_info(|| WidgetInfo::labeled(Role::Link, true, "I-SUCCESS home"));
                    if wordmark.clicked() {
                        nav = Some(Nav::Home);
                    }
                });
            },
        );

        let Self { catalog, subject, open_bands, screen, .. } = self;
        egui::CentralPanel::default().frame(Frame::new().fill(PAGE).inner_margin(Margin::symmetric(24, 18))).show(ui, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(1180.0));
                let catalog = match catalog {
                    Ok(catalog) => catalog,
                    Err(reason) => {
                        ui.heading("The practice catalog could not be loaded");
                        ui.label(reason.as_str());
                        return;
                    }
                };
                let step = match screen {
                    Screen::Home => home(ui, catalog, subject, open_bands),
                    Screen::Grade(grade) => grade_screen(ui, catalog, subject, grade),
                    Screen::Practice(practice) => practice_screen(ui, catalog, practice),
                };
                if step.is_some() {
                    nav = step;
                }
            });
        });

        if let Some(nav) = nav {
            self.go(nav);
        }
    }

    fn go(&mut self, nav: Nav) {
        self.screen = match nav {
            Nav::Home => Screen::Home,
            Nav::Grade(grade) => Screen::Grade(grade),
            Nav::Practice(skill) => match self.catalog.as_ref().ok().and_then(|c| c.skill(&skill)) {
                Some(skill) => Screen::Practice(Practice::new(skill)),
                None => Screen::Home,
            },
        };
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------------------------
// Home: subject pills, bands, grade cards.

fn home(ui: &mut Ui, catalog: &Catalog, subject: &mut String, open_bands: &mut BTreeSet<String>) -> Option<Nav> {
    let mut nav = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        for s in &catalog.subjects {
            let selected = *subject == s.id;
            let (fill, stroke) = if selected { (PILL_ON, PILL_ON) } else { (PILL_OFF, BRAND) };
            let pill = Button::new(RichText::new(&s.name).size(18.0).color(ON_BRAND).strong())
                .fill(fill)
                .stroke(Stroke::new(1.5, stroke))
                .corner_radius(20.0)
                .min_size(vec2(120.0, 40.0))
                .selected(selected);
            let response = ui.add_enabled(s.ready, pill).on_disabled_hover_text(format!("{} is not built yet.", s.name));
            if response.clicked() {
                subject.clone_from(&s.id);
            }
        }
    });
    ui.add_space(14.0);

    let Some(chosen) = catalog.subject(subject) else {
        ui.label("No subject is ready yet.");
        return None;
    };

    let compact = ui.available_width() < 600.0;
    let heading = RichText::new(format!("{} by level", chosen.name)).size(24.0).color(TEXT);
    let mut controls = |ui: &mut Ui| {
        if ui.button("Collapse all").clicked() {
            open_bands.clear();
        }
        if ui.button("Expand all").clicked() {
            open_bands.extend(catalog.bands.iter().map(|b| b.id.clone()));
        }
    };
    if compact {
        ui.label(heading);
        ui.horizontal(&mut controls);
    } else {
        ui.horizontal(|ui| {
            ui.label(heading);
            ui.with_layout(Layout::right_to_left(Align::Center), &mut controls);
        });
    }
    ui.add_space(8.0);

    for band in &catalog.bands {
        let open = open_bands.contains(&band.id);
        let grades: Vec<&Grade> = catalog.band_grades(band).collect();
        let count = if grades.is_empty() {
            "not built yet".to_owned()
        } else {
            format!("{} grade{}", grades.len(), if grades.len() == 1 { "" } else { "s" })
        };
        let label = if compact { format!("{} · {}", band.name, count) } else { format!("{}   ·   {}   ·   {}", band.name, band.range, count) };
        let title = RichText::new(label).size(19.0).color(TEXT);
        let response = CollapsingHeader::new(title).id_salt(("band", band.id.as_str())).open(Some(open)).show(ui, |ui| {
            ui.add_space(6.0);
            if grades.is_empty() {
                if let Some(note) = &band.note {
                    ui.label(RichText::new(note).color(MUTED).italics());
                }
                return;
            }
            let columns = ((ui.available_width() / CARD_MIN_WIDTH).floor() as usize).clamp(1, MAX_COLUMNS);
            for row in grades.chunks(columns) {
                ui.columns(columns, |cols| {
                    for (col, grade) in cols.iter_mut().zip(row) {
                        if let Some(n) = grade_card(col, catalog, chosen, grade) {
                            nav = Some(n);
                        }
                    }
                });
                ui.add_space(12.0);
            }
        });
        response.header_response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &band.name));
        if response.header_response.clicked() {
            if open {
                open_bands.remove(&band.id);
            } else {
                open_bands.insert(band.id.clone());
            }
        }
        ui.add_space(6.0);
    }
    nav
}

/// One grade's card: badge, name, summary, and the chosen subject's skill count.
fn grade_card(ui: &mut Ui, catalog: &Catalog, subject: &Subject, grade: &Grade) -> Option<Nav> {
    let colour = accent(&grade.accent);
    let mut nav = None;
    Frame::new().fill(CARD).stroke(Stroke::new(1.5, colour)).corner_radius(4.0).inner_margin(Margin::same(14)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        // `ui.columns` lays its children out justified, which spreads wrapped words apart.
        ui.with_layout(Layout::top_down(Align::Min), |ui| grade_card_body(ui, catalog, subject, grade, colour, &mut nav));
    });
    nav
}

fn grade_card_body(ui: &mut Ui, catalog: &Catalog, subject: &Subject, grade: &Grade, colour: egui::Color32, nav: &mut Option<Nav>) {
    {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 18.0, colour);
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, &grade.badge, FontId::proportional(17.0), egui::Color32::WHITE);
            ui.add_space(4.0);
            ui.label(RichText::new(&grade.name).size(25.0).color(accent_text(colour)));
        });
        ui.add_space(4.0);
        ui.label(RichText::new(&grade.summary).color(TEXT));
        ui.add_space(4.0);
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(line, 0.0, DIVIDER);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&subject.name).color(TEXT));
            let alignment = if ui.available_width() < 170.0 { Layout::left_to_right(Align::Center) } else { Layout::right_to_left(Align::Center) };
            ui.with_layout(alignment, |ui| {
                let count = catalog.skills_for(&subject.id, &grade.id).count();
                if count == 0 {
                    ui.label(RichText::new("No skills yet").color(MUTED));
                    return;
                }
                let text = format!("{count} skill{} \u{203a}", if count == 1 { "" } else { "s" });
                let link = ui.add(Button::new(RichText::new(text).color(LINK)).frame(false));
                link.widget_info(|| WidgetInfo::labeled(Role::Link, true, format!("{} {} skills", grade.name, subject.name)));
                if link.clicked() {
                    *nav = Some(Nav::Grade(grade.id.clone()));
                }
            });
        });
    }
}

// ---------------------------------------------------------------------------------------------
// A grade's skills.

fn grade_screen(ui: &mut Ui, catalog: &Catalog, subject: &str, grade: &str) -> Option<Nav> {
    let mut nav = None;
    if back(ui, "All levels") {
        nav = Some(Nav::Home);
    }
    let Some(grade) = catalog.grade(grade) else {
        ui.label("This grade is not in the catalog.");
        return nav;
    };
    let colour = accent(&grade.accent);
    let subject_name = catalog.subject(subject).map_or("", |s| s.name.as_str());
    ui.label(RichText::new(format!("{} {}", grade.name, subject_name)).size(30.0).color(accent_text(colour)));
    ui.label(RichText::new(&grade.summary).color(MUTED));
    ui.add_space(12.0);
    Frame::new().fill(CARD).stroke(Stroke::new(1.5, colour)).corner_radius(4.0).inner_margin(Margin::same(16)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let mut any = false;
        for skill in catalog.skills_for(subject, &grade.id) {
            any = true;
            let link = ui.add(Button::new(RichText::new(format!("{}   {}", skill.code, skill.title)).size(18.0).color(LINK)).frame(false));
            link.widget_info(|| WidgetInfo::labeled(Role::Link, true, format!("Practise {} {}", skill.code, skill.title)));
            if link.clicked() {
                nav = Some(Nav::Practice(skill.id.clone()));
            }
        }
        if !any {
            ui.label(RichText::new("No skills yet.").color(MUTED));
        }
    });
    nav
}

// ---------------------------------------------------------------------------------------------
// Practice.

fn practice_screen(ui: &mut Ui, catalog: &Catalog, practice: &mut Practice) -> Option<Nav> {
    let Some(skill) = catalog.skill(practice.skill_id()) else {
        return Some(Nav::Home);
    };
    let grade = catalog.grade(&skill.grade);
    let colour = grade.map_or(LINK, |g| accent(&g.accent));
    let mut nav = None;
    if back(ui, &format!("{} skills", grade.map_or("Grade", |g| g.name.as_str()))) {
        nav = Some(Nav::Grade(skill.grade.clone()));
    }
    ui.label(RichText::new(format!("{}   {}", skill.code, skill.title)).size(30.0).color(accent_text(colour)));
    let (at, of) = practice.position(skill);
    let (correct, tried) = practice.score();
    ui.label(RichText::new(format!("Problem {at} of {of}   \u{00b7}   {correct} correct of {tried} tried")).color(MUTED));
    ui.add_space(12.0);

    Frame::new().fill(CARD).stroke(Stroke::new(1.5, colour)).corner_radius(4.0).inner_margin(Margin::same(20)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        problem_body(ui, skill, practice);
    });

    let shown = practice.steps_shown();
    if shown > 0
        && let Some(problem) = practice.problem(skill)
    {
        ui.add_space(12.0);
        Frame::new().fill(NOTE).stroke(Stroke::new(1.5, NOTE_EDGE)).corner_radius(4.0).inner_margin(Margin::same(14)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Worked steps").strong().color(TEXT));
            for (i, step) in problem.steps.iter().take(shown).enumerate() {
                ui.label(RichText::new(format!("{}.  {}", i + 1, step.say)).color(TEXT));
            }
        });
    }
    nav
}

fn problem_body(ui: &mut Ui, skill: &Skill, practice: &mut Practice) {
    let Some(problem) = practice.problem(skill) else {
        ui.label("This skill has no problems yet.");
        return;
    };
    ui.label(RichText::new(&problem.prompt).size(26.0).color(TEXT));
    ui.add_space(14.0);

    let verdicts: Option<Vec<Verdict>> = practice.verdicts().map(<[Verdict]>::to_vec);
    let mut typed = false;
    for (i, (spec, input)) in problem.answers.iter().zip(practice.inputs.iter_mut()).enumerate() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{}:", spec.label)).size(18.0).color(TEXT));
            if let Some(prefix) = &spec.prefix {
                ui.label(RichText::new(prefix).size(18.0));
            }
            let edge = Frame::new().fill(CARD).stroke(Stroke::new(1.5, BOX_EDGE)).corner_radius(4.0).inner_margin(Margin::symmetric(8, 4));
            let box_ = ui.add(
                TextEdit::singleline(input)
                    .char_limit(4096)
                    .id_salt(("answer", skill.id.as_str(), i))
                    .desired_width(140.0)
                    .font(FontId::proportional(22.0))
                    .frame(edge),
            );
            box_.widget_info(|| WidgetInfo::labeled(Role::TextInput, true, format!("Answer: {}", spec.label)));
            typed |= box_.changed();
            if let Some(suffix) = &spec.suffix {
                ui.label(RichText::new(suffix).size(18.0));
            }
            match verdicts.as_ref().and_then(|v| v.get(i)) {
                Some(Verdict::Correct) => {
                    ui.label(RichText::new("Correct").strong().color(GOOD));
                }
                Some(Verdict::Wrong) => {
                    ui.label(RichText::new("Not yet").strong().color(BAD));
                }
                Some(Verdict::Empty) => {
                    ui.label(RichText::new("Type an answer").color(MUTED));
                }
                None => {}
            }
        });
        ui.add_space(6.0);
    }
    if typed {
        practice.typed();
    }

    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        let check = Button::new(RichText::new("Check").size(17.0).strong().color(ON_PRIMARY)).fill(PRIMARY).min_size(vec2(110.0, 36.0));
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.add(check).clicked() || enter {
            practice.check(skill);
        }
        let more_steps = practice.steps_shown() < problem.steps.len();
        if ui.add_enabled(more_steps, Button::new(RichText::new("Show a step").size(17.0)).min_size(vec2(130.0, 36.0))).clicked() {
            practice.show_step(skill);
        }
        if ui.add(Button::new(RichText::new("Next problem").size(17.0)).min_size(vec2(130.0, 36.0))).clicked() {
            practice.next(skill);
        }
    });
    if practice.solved() {
        ui.add_space(10.0);
        ui.label(RichText::new("Well done! That is right.").size(20.0).strong().color(GOOD));
    }
}

/// A "back" link. Returns whether it was clicked.
fn back(ui: &mut Ui, to: &str) -> bool {
    let link = ui.add(Button::new(RichText::new(format!("\u{2039} {to}")).size(16.0).color(LINK)).frame(false));
    link.widget_info(|| WidgetInfo::labeled(Role::Link, true, format!("Back to {to}")));
    ui.add_space(6.0);
    link.clicked()
}

#[cfg(test)]
mod practice_files {
    use super::{App, Screen};
    use i_success_engine::Practice;

    #[test]
    fn restores_checked_answers_and_preserves_the_current_session_on_error() -> Result<(), String> {
        let catalog = i_success_content::builtin().map_err(|e| e.to_string())?;
        let skill = catalog.skills.first().ok_or("missing practice skill")?;
        let mut practice = Practice::new(skill);
        let problem = practice.problem(skill).ok_or("missing problem")?;
        practice.inputs = problem.answers.iter().map(|a| a.answer.clone()).collect();
        practice.check(skill);
        assert!(practice.solved());
        let mut app = App::new();
        app.set_screen(Screen::Practice(practice));
        let saved = app.document()?;
        let mut restored = App::new();
        restored.load_document(&saved)?;
        assert_eq!(app.screen(), restored.screen());
        let damaged = saved.replace("index = 0", "index = 999999");
        assert_ne!(damaged, saved);
        assert!(restored.load_document(&damaged).is_err());
        assert_eq!(restored.document()?, saved);
        Ok(())
    }
}
