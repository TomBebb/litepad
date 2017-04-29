extern crate gtk;
extern crate uuid;
extern crate pulldown_cmark;

use gtk::*;
use uuid::Uuid;
use std::io::{Read, Write};
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pulldown_cmark::{Parser, Event, Tag};

const TITLE: &str = "Litepad";
const H1_SCALE: f64 = 2.;
const H2_SCALE: f64 = 1.6;
const H3_SCALE: f64 = 1.2;

#[derive(Clone)]
pub struct View {
    pub label: Label,
    pub text: TextBuffer,
    pub view: TextView,
    pub uuid: Uuid,
    pub file_path: Arc<Mutex<Option<PathBuf>>>,
}
impl View {
    pub fn new(path: Option<PathBuf>, tags: &TextTagTable) -> View {
        let buffer = TextBuffer::new(Some(tags));
        let view = TextView::new_with_buffer(&buffer);
        let view = View {
            label: Label::new("..."),
            text: buffer,
            uuid: Uuid::new_v4(),
            view,
            file_path: Arc::new(Mutex::new(path))
        };
        view.update_title();
        view
    }
    pub fn setup(&self, app: &App)
    {
        let hbox = Box::new(Orientation::Horizontal, 2);
        hbox.add(&self.view);
        let event_box = EventBox::new();
        event_box.add(&self.label);
        let app2 = app.clone();
        let id = self.uuid.clone();
        event_box.connect_button_press_event(move |me, ev| {
            if ev.get_button() == 3 {
                // Load menu
                let glade_src = include_str!("../menu.glade");
                // Build from glade
                let builder = Builder::new_from_string(glade_src);
                let menu: Menu = builder.get_object("menu").unwrap();
                let close_tab: MenuItem = builder.get_object("close-tab").unwrap();
                let app = app2.clone();
                let id = id.clone();
                close_tab.connect_select(move |_| {
                    {
                        let mut views = app.views.try_lock().unwrap();
                        if let Some(index) = views.iter().enumerate().find(|&(_, ref v)| v.uuid == id).map(|(i, _)| i) {
                            views.remove(index);
                            app.tabs.remove_page(Some(index as u32));
                        }
                    }
                    app.update_title();
                });
                // Pop it up
                menu.popup(None::<&Widget>, Some(me), |_, _, _| {true}, 0, ev.get_time());
            }
            Inhibit(false)
        });
        app.tabs.append_page(&hbox, Some(&event_box));
        event_box.show_all();
        app.tabs.set_current_page(None);
        hbox.show_all();
    }

    pub fn open(path: PathBuf, tags: &TextTagTable) -> View {
        let view = View::new(Some(path.clone()), tags);
        let path = path.as_path();
        let mut file = File::open(path).unwrap();
        let mut orig_text = String::new();
        file.read_to_string(&mut orig_text).unwrap();
        let parser = Parser::new(&orig_text);
        let mut tag_starts = Vec::with_capacity(4);
        let mut tag_defs = Vec::new();
        let mut row = 0;
        let mut column = 0;
        let mut text = String::with_capacity(orig_text.len());
        for event in parser {
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
                        Tag::Emphasis => Some("italic"),
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
        view.text.set_text(&text);
        for (tag, start_row, start_column, row, column) in tag_defs {
            let start = view.text.get_iter_at_line_index(start_row, start_column);
            let end = view.text.get_iter_at_line_index(row, column);
            view.text.apply_tag_by_name(tag, &start, &end);
        }
        view
    }
    pub fn apply_line_tag(&self, tag: &TextTag) {
        if let Some((start, end)) = self.text.get_selection_bounds() {
            let mut line_end = end.clone();
            line_end.forward_line();
            line_end.backward_char();
            let mut line_start = start.clone();
            line_start.backward_line();
            let mut iter = line_start.clone();
            if iter.forward_to_tag_toggle(Some(tag)) && iter <= line_end {
                self.text.remove_tag(tag, &line_start, &line_end);
            } else {
                self.text.apply_tag(tag, &line_start, &line_end);
            }
        }
    }
    pub fn apply_plain_tag(&self, tag: &TextTag) {
        if let Some((start, end)) = self.text.get_selection_bounds() {
            let mut iter = start.clone();
            if iter.forward_to_tag_toggle(Some(tag)) && iter <= end {
                self.text.remove_tag(tag, &start, &end);
            } else {
                self.text.apply_tag(tag, &start, &end);
            }
        }
    }
    pub fn save(&self, new_path: Option<PathBuf>) {
        let mut path = self.file_path.try_lock().unwrap();
        *path = path.clone().or(new_path);
        let buffer = &self.text;
        let (start, end) = buffer.get_bounds();
        if let Some(text) = buffer.get_text(&start, &end, true) {
            if let Some(path) = path.as_ref() {
                let mut iter = buffer.get_start_iter();
                let table = buffer.get_tag_table().unwrap();
                let bold = table.lookup("bold").unwrap();
                let italic = table.lookup("italic").unwrap();
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
                    } else if iter.toggles_tag(Some(&italic)) {
                        new_text.push_str("*");
                        if iter.begins_tag(Some(&italic)) {
                            unclosed_tags.push("*");
                        } else {
                            assert_eq!(unclosed_tags.pop(), Some("*"));
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
                let mut file = File::create(path).unwrap();
                file.write_all(new_text.as_bytes()).unwrap();
            }
        }
        self.text.set_modified(false);
    }
    pub fn update_title(&self) -> String {
        let title = self.get_title();
        self.label.set_text(&title);
        title
    }
    pub fn get_title(&self) -> String {
        let path = self.file_path.try_lock().unwrap();
        let title = path.as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or(String::from("Untitled"));
        let symbol = if self.text.get_modified() {
            "*"
        } else {
            ""
        };
        format!("{}{}", title, symbol)
    }
}

#[derive(Clone)]
pub struct App {
    tags: TextTagTable,
    pub window: Window,
    pub italic: ToolButton,
    pub bold: ToolButton,
    pub h1: ToolButton,
    pub new: ToolButton,
    pub open: ToolButton,
    pub save: ToolButton,
    pub tabs: Notebook,
    pub views: Arc<Mutex<Vec<View>>>,
}
impl App {
    pub fn current_view(&self) -> usize {
        self.tabs.get_property_page() as usize
    }
    /// Set up the app
    pub fn new(builder: Builder) -> App {
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
        let italic = TextTag::new("italic");
        bold.set_property_style_set(true);
        tags.add(&italic);
        let default_view = View::new(None, &tags);
        App {
            tags,
            window: builder.get_object("window").unwrap(),
            h1: builder.get_object("h1").unwrap(),
            bold: builder.get_object("bold").unwrap(),
            italic: builder.get_object("italic").unwrap(),
            tabs: builder.get_object("tabs").unwrap(),
            new: builder.get_object("new").unwrap(),
            open: builder.get_object("open").unwrap(),
            save: builder.get_object("save").unwrap(),
            views: Arc::new(Mutex::new(vec![default_view])),
        }
    }
    pub fn update_title(&self) {
        let views = self.views.try_lock().unwrap();
        if let Some(view) = views.get(self.current_view()) {
            let title = view.update_title();
            self.window.set_title(&title);
        }
    }
    pub fn open(&self, path: PathBuf) {
        let mut views = self.views.try_lock().unwrap();
        let view = View::open(path, &self.tags);
        view.setup(self);
        //self.tabs.set_property_page(views.len() as i32);
        views.push(view);
    }
    pub fn setup(&self) {
        self.tabs.remove_page(None);
        {
            let views = self.views.try_lock().unwrap();
            views[0].setup(self);
        };
        let filter = FileFilter::new();
        filter.add_pattern("*.md");
        filter.add_pattern("*.txt");
        filter.add_pattern("*.markdown");
        filter.add_mime_type("text/markdown");
        filter.set_name("Markdown");
        let me = self.clone();
        self.new.connect_clicked(move |_| {
            let mut views = me.views.try_lock().unwrap();
            let view = View::new(None, &me.tags);
            view.setup(&me);
            views.push(view);
        });
        let window = self.window.clone();
        let filter2 = filter.clone();
        let save = self.save.clone();
        let views = self.views.clone();
        let tabs = self.tabs.clone();
        let me = self.clone();
        save.connect_clicked(move |_| {
            let path = {
                let views = views.try_lock().unwrap();
                views
                    .get(tabs.get_property_page() as usize)
                    .and_then(|v| v.file_path.try_lock().unwrap().clone())
            };
            if path.is_some() {
                let views = views.try_lock().unwrap();
                views[tabs.get_property_page() as usize].save(None);
            } else {
                let dialog = FileChooserDialog::new(Some("Select a file"),
                                                    Some(&window),
                                                    FileChooserAction::Save);
                dialog.add_button("Save", 0);
                dialog.add_button("Cancel", 1);
                dialog.add_filter(&filter2);
                let me = me.clone();
                dialog.connect_response(move |dialog, id| {
                    let me = me.clone();
                    if id == 0 {
                        if let Some(filename) = dialog.get_filename() {
                            let views = me.views.try_lock().unwrap();
                            if let Some(view) = views.get(me.current_view()) {
                                let mut path = view.file_path.try_lock().unwrap();
                                *path = Some(filename);
                            }
                        }
                    }
                    me.update_title();
                    dialog.destroy();
                });
                dialog.show_all();
                dialog.run();
            }
            me.update_title();
        });
        let me = self.clone();
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
                let me = me.clone();
                dialog.connect_response(move |dialog, id| {
                    if id == 0 {
                        if let Some(filename) = dialog2.get_filename() {
                            me.open(filename);
                        }
                        me.update_title();
                    }
                    dialog.destroy();
                });
                dialog.show_all();
                dialog.run();
            });
        let me = self.clone();
        self.bold
            .connect_clicked(move |_| {
                                 let views = me.views.try_lock().unwrap();
                                 if let Some(view) = views.get(me.current_view()) {
                                     view.apply_plain_tag(&me.tags.lookup("bold").unwrap());
                                 }
                             });
        let me = self.clone();
        self.h1
            .connect_clicked(move |_| {
                                 let views = me.views.try_lock().unwrap();
                                 if let Some(view) = views.get(me.current_view()) {
                                     view.apply_line_tag(&me.tags.lookup("h1").unwrap());
                                 }
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
