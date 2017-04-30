use app::App;
use source::Source;
use std::collections::HashMap;
use std::io::{BufReader, Read, Error};
use std::sync::{Arc, Mutex};

use pulldown_cmark::{Parser, Event, Tag};

use gdk_pixbuf::{Pixbuf, PixbufLoader, InterpType};

use hyper::Url;

use util;

use gtk::*;

use webbrowser;

fn is_block(tag: &Tag) -> bool {
    match *tag {
        Tag::Header(_) |
        Tag::CodeBlock(_) | 
        Tag::Item => true,
        _ => false,
    }
}

fn load_pixbufs(urls: &[Url], max_width: i32) -> Vec<Option<Pixbuf>> {
    let client = util::make_client();
    let mut bytes = Vec::with_capacity(512);
    urls.iter()
        .map(|url| {
            let loader = PixbufLoader::new();
            if let Ok(input) = client.get(url.clone()).send() {
                let mut reader = BufReader::new(input);
                bytes.clear();
                reader.read_to_end(&mut bytes).unwrap();
                loader.loader_write(&bytes).unwrap();
                loader.close().unwrap();
                let mut image = loader.get_pixbuf().unwrap();
                let (width, height) = (image.get_width(), image.get_height());
                if width > max_width {
                    let (new_width, new_height) = (max_width, (height * max_width) / width);
                    image = image
                        .scale_simple(new_width, new_height, InterpType::Bilinear)
                        .unwrap();
                }
                Some(image)
            } else {
                None
            }
        })
        .collect()
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
        let mut tag_starts = Vec::with_capacity(4);
        let mut tag_defs = Vec::new();
        let mut row = 0;
        let mut column = 0;
        let mut text = String::with_capacity(orig_text.len());
        let mut image_urls = Vec::new();
        let mut image_places = Vec::new();
        let mut links = Vec::new();
        let mut in_image = false;
        for event in parser {
            println!("{:?}", event);
            match event {
                Event::SoftBreak |
                Event::HardBreak |
                Event::End(Tag::Paragraph) => {
                    text.push('\n');
                    row += 1;
                    column = 0;
                },
                Event::Start(tag) => {
                    match tag {
                        Tag::Image(_, _) => {
                            in_image = true;
                        },
                        Tag::Item => {
                            text.push_str("• ");
                            column += 2;
                        },
                        _ => ()
                    }
                    tag_starts.push((tag, row, column))
                },
                Event::End(Tag::Image(ref url, _)) => {
                    image_places.push((image_urls.len(), row, column));
                    image_urls.push(Url::parse(&url).unwrap());
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
                            links.push(url);
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
        let pixbufs = load_pixbufs(&image_urls, 500);
        view.text.set_text(&text);
        {
            let mut new_links = view.links.lock().unwrap();
            let mut link = 0;
            for (tag, start_row, start_column, row, column) in tag_defs {
                let start = view.text.get_iter_at_line_index(start_row, start_column);
                let end = view.text.get_iter_at_line_index(row, column);
                view.text.apply_tag_by_name(tag, &start, &end);
                if tag == "link" {
                    new_links.push(MetaIter {
                                       start: start,
                                       end: end,
                                       data: links[link].to_string(),
                                   });
                    link += 1;
                }
            }
        }
        {
            let mut actual_image_urls = view.image_urls.lock().unwrap();
            for (index, row, column) in image_places {
                if let Some(ref pixbuf) = pixbufs[index] {
                    actual_image_urls.insert(pixbuf.clone(), image_urls[index].clone());
                    let mut place = view.text.get_iter_at_line_index(row, column);
                    view.text.insert_pixbuf(&mut place, pixbuf);
                }
            }
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
