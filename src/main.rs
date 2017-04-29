extern crate gtk;
extern crate pulldown_cmark;

use gtk::*;
use std::io::{Read, Write};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use pulldown_cmark::{Parser, Event, Tag};

const TITLE: &str = "Litepad";
const H1_SCALE: f64 = 2.;
const H2_SCALE: f64 = 1.6;
const H3_SCALE: f64 = 1.2;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct FileDetail {
    pub path: PathBuf,
    pub changed: bool,
}

#[derive(Clone)]
pub struct App {
    pub window: Window,
    pub italic: ToolButton,
    pub bold: ToolButton,
    pub open: ToolButton,
    pub save: ToolButton,
    pub text: TextView,
    file: Arc<Mutex<Option<FileDetail>>>,
}
impl App {
    /// Set up the app
    pub fn new(builder: Builder) -> App {
        App {
            window: builder.get_object("window").unwrap(),
            bold: builder.get_object("bold").unwrap(),
            italic: builder.get_object("italic").unwrap(),
            text: builder.get_object("text").unwrap(),
            open: builder.get_object("open").unwrap(),
            save: builder.get_object("save").unwrap(),
            file: Arc::new(Mutex::new(None::<FileDetail>)),
        }
    }
    pub fn update_title(&self, detail: Option<&FileDetail>) {
        match detail {
            None => self.window.set_title(TITLE),
            Some(&FileDetail { ref path, changed }) => {
                let symbol = if changed { "*" } else { "" };
                self.window
                    .set_title(&format!("{}{} - {}", path.display(), symbol, TITLE))
            }
        };
    }
    pub fn save(&self) {
        let mut file = self.file.lock().unwrap();
        if let Some(mut detail) = file.as_mut() {
            let buffer = self.text.get_buffer().unwrap();
            let (start, end) = buffer.get_bounds();
            if let Some(text) = buffer.get_text(&start, &end, true) {
                let mut iter = buffer.get_start_iter();
                let table = buffer.get_tag_table().unwrap();
                let bold = table.lookup("bold").unwrap();
                let h1 = table.lookup("h1").unwrap();
                let h2 = table.lookup("h2").unwrap();
                let mut new_text = String::with_capacity(text.len());
                let mut unclosed_tags = Vec::new();
                loop {
                    if iter.begins_tag(Some(&h1)) {
                        new_text.push_str("# ");
                    } else if iter.begins_tag(Some(&h2)) {
                        new_text.push_str("## ");
                    } else if iter.toggles_tag(Some(&bold)) {
                        new_text.push_str("**");
                        if iter.begins_tag(Some(&bold)) {
                            unclosed_tags.push("**");
                        } else {
                            assert_eq!(unclosed_tags.pop(), Some("**"));
                        }
                    }
                    if let Some(ch) = iter.get_char() {
                        if ch == '\n' {
                            new_text.push(ch);
                        }
                        new_text.push(ch);
                    }
                    if !iter.forward_char() {
                        break;
                    }
                }
                for tag in unclosed_tags {
                    new_text.push_str(tag)
                }
                let path = detail.path.as_path();
                let mut file = File::create(path).unwrap();
                file.write_all(new_text.as_bytes()).unwrap();
            }
            detail.changed = false;
        }
    }
    pub fn load(&self) {
        let path = {
            let file = self.file.lock().unwrap();
            file.as_ref().map(|f| f.path.clone())
        };
        if let Some(path) = path {
            let path = path.as_path();
            let mut file = File::open(path).unwrap();
            let mut orig_text = String::new();
            file.read_to_string(&mut orig_text).unwrap();
            let buffer = self.text.get_buffer().unwrap();
            let parser = Parser::new(&orig_text);
            let mut tag_starts = Vec::with_capacity(4);
            let mut tag_defs = Vec::new();
            let mut row = 0;
            let mut column = 0;
            let mut text = String::with_capacity(orig_text.len());
            for event in parser {
                println!("{:?}", event);
                match event {
                    Event::SoftBreak |
                    Event::HardBreak |
                    Event::End(Tag::Paragraph) => {
                        text.push('\n');
                        row += 1;
                        column = 0;
                    }
                    Event::Start(tag) => tag_starts.push((tag, row, column)),
                    Event::End(_) => {
                        let (tag, start_row, start_column) = tag_starts.pop().unwrap();
                        let name = match tag {
                            Tag::Header(n) => {
                                text.push('\n');
                                column = 0;
                                row += 1;
                                Some(match n {
                                         1 => "h1",
                                         2 => "h2",
                                         _ => "h3",
                                     })
                            }
                            Tag::Strong => Some("bold"),
                            _ => None,
                        };
                        if let Some(name) = name {
                            tag_defs.push((name, start_row, start_column, row, column));
                        }
                    }
                    Event::Text(ref etext) => {
                        text.push_str(&**etext);
                        column += etext.len() as i32
                    }
                    _ => (),
                }
            }
            buffer.set_text(&text);
            for (tag, start_row, start_column, row, column) in tag_defs {
                let start = buffer.get_iter_at_line_index(start_row, start_column);
                let end = buffer.get_iter_at_line_index(row, column);
                println!("{:?}: {:?}", tag, buffer.get_slice(&start, &end, false));
                buffer.apply_tag_by_name(tag, &start, &end);
            }
            if let Some(mut detail) = self.file.lock().unwrap().as_mut() {
                detail.changed = false;
            }
        }
    }
    pub fn setup(&self) {
        let filter = FileFilter::new();
        filter.add_pattern("*.md");
        filter.add_pattern("*.txt");
        filter.add_pattern("*.markdown");
        filter.add_mime_type("text/markdown");
        filter.set_name("Markdown");
        let tags = TextTagTable::new();
        let h1 = TextTag::new("h1");
        h1.set_property_weight(700);
        h1.set_property_scale(H1_SCALE);
        tags.add(&h1);
        let h2 = TextTag::new("h2");
        h2.set_property_weight(700);
        h2.set_property_scale(H2_SCALE);
        tags.add(&h2);
        let h3 = TextTag::new("h3");
        h3.set_property_weight(700);
        h3.set_property_scale(H3_SCALE);
        tags.add(&h3);
        let bold = TextTag::new("bold");
        bold.set_property_weight(700);
        tags.add(&bold);
        let buffer = TextBuffer::new(Some(&tags));
        self.text.set_buffer(Some(&buffer));
        let file2 = self.file.clone();
        let window2 = self.window.clone();
        let filter2 = filter.clone();
        let save = self.save.clone();
        let me = self.clone();

        save.connect_clicked(move |_| {
            let window = &window2;
            let exists = {
                let f = file2.lock().unwrap();
                f.is_some()
            };
            if exists {
                me.save();
                {
                    let file = file2.lock().unwrap();
                    me.update_title(Some(&file.as_ref().unwrap()));
                }
            } else {
                let dialog = FileChooserDialog::new(Some("Select a file"),
                                                    Some(window),
                                                    FileChooserAction::Save);
                dialog.add_button("Save", 0);
                dialog.add_button("Cancel", 1);
                dialog.add_filter(&filter2);
                let file2 = file2.clone();
                let me = me.clone();
                dialog.connect_response(move |dialog, id| {
                    let me = me.clone();
                    if id == 0 {
                        if let Some(filename) = dialog.get_filename() {
                            let detail = FileDetail {
                                path: filename.clone(),
                                changed: false,
                            };
                            me.save();
                            me.update_title(Some(&detail));
                            {
                                let mut file = file2.lock().unwrap();
                                *file = Some(detail);
                            }
                        }
                    }
                    dialog.destroy();
                });
                dialog.show_all();
                dialog.run();
            }
        });
        let me = self.clone();
        let file2 = self.file.clone();
        let window2 = self.window.clone();
        self.open
            .connect_clicked(move |_| {
                let window = &window2;
                let dialog = FileChooserDialog::new(Some("Select a file"),
                                                    Some(window),
                                                    FileChooserAction::Open);
                dialog.add_button("Open", 0);
                dialog.add_button("Cancel", 1);
                dialog.add_filter(&filter);
                let dialog2 = dialog.clone();
                let file2 = file2.clone();
                let me = me.clone();
                dialog.connect_response(move |dialog, id| {
                    if id == 0 {
                        if let Some(filename) = dialog2.get_filename() {
                            let detail = FileDetail {
                                path: filename,
                                changed: false,
                            };
                            me.update_title(Some(&detail));
                            {
                                let mut file = file2.lock().unwrap();
                                *file = Some(detail);
                            }
                            me.load();
                        }
                    }
                    dialog.destroy();
                });
                dialog.show_all();
                dialog.run();
            });
        let text = self.text.clone();
        self.bold
            .connect_clicked(move |_| {
                let buffer = text.get_buffer().unwrap();
                if let Some((start, end)) = buffer.get_selection_bounds() {
                    let mut iter = start.clone();
                    if iter.forward_to_tag_toggle(Some(&bold)) && iter <= end {
                        buffer.remove_tag(&bold, &start, &end);
                    } else {
                        buffer.apply_tag(&bold, &start, &end);
                    }
                }
            });
        let user_actions = Arc::new(AtomicUsize::new(0));
        let file2 = self.file.clone();
        let me = self.clone();
        let user_actions2 = user_actions.clone();
        buffer.connect_changed(move |_| {
            let mut file = file2.lock().unwrap();
            if let Some(mut detail) = file.as_mut() {
                if user_actions2.load(Ordering::Relaxed) > 0 {
                    println!("changed");
                    detail.changed = true;
                    me.update_title(Some(detail));
                }
            }
        });
        let user_actions2 = user_actions.clone();
        buffer.connect_begin_user_action(move |_| {

                                             println!("{}",
                                                      user_actions2.fetch_add(1,
                                                                              Ordering::Relaxed));
                                         });
        buffer.connect_end_user_action(move |_| {
                                           println!("{}",
                                                    user_actions.fetch_sub(1, Ordering::Relaxed));
                                       });
    }
}

fn main() {
    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }
    // Save glade file as constant
    let glade_src = include_str!("../ui.glade");
    // Build from glade
    let builder = gtk::Builder::new_from_string(glade_src);
    let app = App::new(builder);
    app.setup();
    // Show the window
    app.window.show_all();

    app.window
        .connect_delete_event(|_, _| {
                                  gtk::main_quit();
                                  Inhibit(false)
                              });
    // Start running main loop
    gtk::main();
}