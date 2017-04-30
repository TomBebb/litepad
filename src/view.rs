use app::App;
use source::Source;
use std::collections::HashMap;
use std::io::Error;
use std::sync::{Arc, Mutex};

use pulldown_cmark::{Parser, Event, Tag};

use hyper::Url;

use util;

use gdk_pixbuf::Pixbuf;

use gtk::*;
use std::boxed::Box;

use webbrowser;

fn is_block(tag: &Tag) -> bool {
    match *tag {
        Tag::Header(_) |
        Tag::CodeBlock(_) |
        Tag::Item => true,
        _ => false,
    }
}
#[derive(Clone)]
pub struct MetaIter<T> {
    pub start: TextIter,
    pub end: TextIter,
    pub data: T,
}

#[derive(Clone)]
pub struct View {
    pub label: Label,
    pub links: Arc<Mutex<Vec<MetaIter<String>>>>,
    pub text: TextBuffer,
    pub view: TextView,
    pub window: ScrolledWindow,
    pub source: Arc<Mutex<Source>>,
    pub image_urls: Arc<Mutex<HashMap<Pixbuf, Url>>>,
}

impl View {
    pub fn new(source: Source, tags: &TextTagTable) -> View {
        let buffer = TextBuffer::new(Some(tags));
        let view = TextView::new_with_buffer(&buffer);
        let window = ScrolledWindow::new(None, None);
        View {
            label: Label::new(format!("{}", source).as_str()),
            text: buffer,
            links: Arc::new(Mutex::new(Vec::new())),
            view,
            window,
            source: Arc::new(Mutex::new(source)),
            image_urls: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn setup(&self, app: &App) {
        self.window.add(&self.view);
        let event_box = EventBox::new();
        event_box.add(&self.label);
        let links = self.links.clone();
        self.view
            .connect_button_press_event(move |text, ev| {
                let (x, y) = ev.get_position();
                let links = links.lock().unwrap();
                if let Some(iter) = text.get_iter_at_location(x as i32, y as i32) {
                    for link in links.iter() {
                        if link.start <= iter && link.end >= iter {
                            webbrowser::open(&link.data).unwrap();
                            break;
                        }
                    }
                }
                Inhibit(false)
            });
        let app2 = app.clone();
        let source = self.source.clone();
        event_box.connect_button_press_event(move |me, ev| {
            if ev.get_button() == 3 {
                // Load menu
                let glade_src = include_str!("../tab-menu.glade");
                // Build from glade
                let builder = Builder::new_from_string(glade_src);
                let menu: Menu = builder.get_object("menu").unwrap();
                let close_tab: MenuItem = builder.get_object("close-tab").unwrap();
                let app = app2.clone();
                let source = source.clone();
                close_tab.connect_activate(move |_| {
                    let index = {
                        let mut views = app.views.lock().unwrap();
                        if let Some(index) = views
                               .iter()
                               .enumerate()
                               .find(|&(_, ref v)| {
                                         *v.source.lock().unwrap() == *source.lock().unwrap()
                                     })
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
        app.tabs.append_page(&self.window, Some(&event_box));
        event_box.show_all();
        app.tabs.set_current_page(None);
        self.window.show_all();
    }

    pub fn open(source: Source, tags: &TextTagTable) -> View {
        let orig_text = source.load();
        let view = View::new(source, tags);
        let parser = Parser::new(&orig_text);
        let mut post_ops: Vec<Box<Fn(&TextView, &TextBuffer, &[Option<Pixbuf>])>> =
            Vec::with_capacity(4);
        let mut tag_starts = Vec::with_capacity(4);
        let mut tag_defs = Vec::new();
        let mut row = 0;
        let mut column = 0;
        let mut text = String::with_capacity(orig_text.len());
        let mut in_image = false;
        let mut image_urls = Vec::new();
        for event in parser {
            println!("{:?}", event);
            match event {
                Event::HardBreak |
                Event::End(Tag::Paragraph) => {
                    text.push('\n');
                    row += 1;
                    column = 0;
                }
                Event::End(Tag::Rule) => {
                    let (sep_row, sep_column) = (row, column);
                    post_ops.push(Box::new(move |view, buf, _| {
                        let mut iter = buf.get_iter_at_line_index(sep_row, sep_column);
                        if let Some(anchor) = buf.create_child_anchor(&mut iter) {
                            let sep = Separator::new(Orientation::Horizontal);
                            view.add_child_at_anchor(&sep, &anchor);
                        }

                    }));
                }
                Event::Start(tag) => {
                    match tag {
                        Tag::Image(_, _) => {
                            in_image = true;
                        }
                        Tag::Item => {
                            text.push_str("• ");
                            column += 2;
                        }
                        _ => (),
                    }
                    tag_starts.push((tag, row, column))
                }
                Event::End(Tag::Image(ref url, _)) => {
                    let url = Url::parse(&url).unwrap();
                    let (img_row, img_col) = (row, column);
                    let index = image_urls.len();
                    image_urls.push(url.clone());
                    let me_image_urls = view.image_urls.clone();
                    post_ops.push(Box::new(move |_, buf, pixbufs| {
                        let mut actual_image_urls = me_image_urls.lock().unwrap();
                        // If the image has loaded
                        if let Some(ref pixbuf) = pixbufs[index] {
                            actual_image_urls.insert(pixbuf.clone(), url.clone());
                            let mut place = buf.get_iter_at_line_index(img_row, img_col);
                            buf.insert_pixbuf(&mut place, pixbuf);
                        }
                    }));
                    in_image = false;
                }
                Event::End(_) => {
                    let (tag, start_row, start_column) = tag_starts.pop().unwrap();
                    if is_block(&tag) {
                        text.push('\n');
                    }
                    let name = match tag {
                        Tag::Code |
                        Tag::CodeBlock(_) => Some("code"),
                        Tag::Item => Some("item"),
                        Tag::Header(n) => {
                            column = 0;
                            row += 1;
                            Some(match n {
                                     1 => "h1",
                                     2 => "h2",
                                     _ => "h3",
                                 })
                        }
                        Tag::Link(url, _) => {
                            let links = view.links.clone();
                            let (end_row, end_col) = (row, column);
                            post_ops.push(Box::new(move |_, buf, _| {
                                let mut new_links = links.lock().unwrap();
                                let start = buf.get_iter_at_line_index(start_row, start_column);
                                let end = buf.get_iter_at_line_index(end_row, end_col);
                                buf.apply_tag_by_name("link", &start, &end);
                                new_links.push(MetaIter {
                                                   start: start,
                                                   end: end,
                                                   data: url.to_string(),
                                               });
                            }));
                            Some("link")
                        }
                        Tag::Strong => Some("bold"),
                        Tag::Emphasis => Some("italic"),
                        _ => None,
                    };
                    if let Some(name) = name {
                        tag_defs.push((name, start_row, start_column, row, column));
                    }
                }
                Event::Text(ref etext) if !in_image => {
                    text.push_str(&**etext);
                    column += etext.len() as i32
                }
                _ => (),
            }
        }
        let pixbufs = util::load_pixbufs(&image_urls, 500);
        view.text.set_text(&text);
        for op in post_ops {
            op(&view.view, &view.text, pixbufs.as_slice());
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
    pub fn save(&self, new_source: Source) -> Result<(), Error> {
        let mut source = self.source.lock().unwrap();
        if *source == Source::Unknown {
            *source = new_source;
        }
        let buffer = &self.text;
        if let Some(mut writer) = source.writer() {
            let mut iter = buffer.get_start_iter();
            let table = buffer.get_tag_table().unwrap();
            let bold = table.lookup("bold").unwrap();
            let italic = table.lookup("italic").unwrap();
            let h1 = table.lookup("h1").unwrap();
            let h2 = table.lookup("h2").unwrap();
            let mut unclosed_tags = Vec::new();
            let urls = self.image_urls.lock().unwrap();
            loop {
                if let Some(pixbuf) = iter.get_pixbuf() {
                    let url = &urls[&pixbuf];
                    writer
                        .write_all(format!("![]({})", url).as_bytes())
                        .unwrap();
                } else if iter.begins_tag(Some(&h1)) {
                    writer.write_all(b"# ")?;
                } else if iter.begins_tag(Some(&h2)) {
                    writer.write_all(b"## ")?;
                } else if iter.toggles_tag(Some(&bold)) {
                    writer.write_all(b"**")?;
                    if iter.begins_tag(Some(&bold)) {
                        unclosed_tags.push("**");
                    } else {
                        debug_assert_eq!(unclosed_tags.pop(), Some("**"));
                    }
                } else if iter.toggles_tag(Some(&italic)) {
                    writer.write_all(b"*")?;
                    if iter.begins_tag(Some(&italic)) {
                        unclosed_tags.push("*");
                    } else {
                        debug_assert_eq!(unclosed_tags.pop(), Some("*"));
                    }
                }
                if let Some(ch) = iter.get_char() {
                    if ch == '\n' {
                        writer.write_all(b"\n")?;
                    }
                    writer.write_all(&ch.to_string().as_bytes())?;
                }
                if !iter.forward_char() {
                    break;
                }
            }
            for tag in unclosed_tags {
                writer.write_all(tag.as_bytes())?;
            }
        }
        self.text.set_modified(false);
        Ok(())
    }
    pub fn update_title(&self) -> String {
        let title = self.get_title();
        self.label.set_text(&title);
        title
    }
    pub fn get_title(&self) -> String {
        let source = self.source.lock().unwrap();
        let title = format!("{}", *source);
        let symbol = if self.text.get_modified() { "*" } else { "" };
        format!("{}{}", title, symbol)
    }
}
