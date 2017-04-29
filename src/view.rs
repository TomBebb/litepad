use app::App;

use gtk::*;

use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pulldown_cmark::{Parser, Event, Tag};

use uuid::Uuid;


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
            file_path: Arc::new(Mutex::new(path)),
        };
        view.update_title();
        view
    }
    pub fn setup(&self, app: &App) {
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
                    let index = {
                        let mut views = app.views.try_lock().unwrap();
                        if let Some(index) = views
                               .iter()
                               .enumerate()
                               .find(|&(_, ref v)| v.uuid == id)
                               .map(|(i, _)| i) {
                            views.remove(index);
                            Some(index)
                        } else {
                            None
                        }
                    };
                    if let Some(index) = index {
                        app.tabs.remove_page(Some(index as u32));
                    }
                });
                // Pop it up
                menu.popup(None::<&Widget>, Some(me), |_, _, _| true, 0, ev.get_time());
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
        let title = if let Some(path) = path.as_ref() {
            let mut text: String = path.display().to_string();
            if let Some(home) = env::home_dir() {
                if path.starts_with(&home) {
                    text = text.replace(&home.display().to_string(), "~");
                }
            }
            text
        } else {
            "Untitled".into()
        };
        let symbol = if self.text.get_modified() { "*" } else { "" };
        format!("{}{}", title, symbol)
    }
}
