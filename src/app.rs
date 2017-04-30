use source::Source;
use view::View;

use gtk::*;

use std::sync::{Arc, Mutex};

use hyper::Url;


const TITLE: &str = "Litepad";
const H1_SCALE: f64 = 2.;
const H2_SCALE: f64 = 1.6;
const H3_SCALE: f64 = 1.2;

#[derive(Clone)]
pub struct App {
    tags: TextTagTable,
    pub window: Window,
    pub italic: ToolButton,
    pub bold: ToolButton,
    pub h1: ToolButton,
    pub h2: ToolButton,
    pub new: ToolButton,
    pub open: ToolButton,
    pub save: ToolButton,
    pub close: ToolButton,
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
        let link = TextTag::new("link");
        link.set_property_foreground(Some("blue"));
        link.set_property_underline_set(true);
        tags.add(&link);
        let code = TextTag::new("code");
        code.set_property_font(Some("Courier New"));
        tags.add(&code);
        App {
            tags,
            window: builder.get_object("window").unwrap(),
            h1: builder.get_object("h1").unwrap(),
            h2: builder.get_object("h2").unwrap(),
            bold: builder.get_object("bold").unwrap(),
            italic: builder.get_object("italic").unwrap(),
            tabs: builder.get_object("tabs").unwrap(),
            new: builder.get_object("new").unwrap(),
            open: builder.get_object("open").unwrap(),
            save: builder.get_object("save").unwrap(),
            close: builder.get_object("close").unwrap(),
            views: Arc::new(Mutex::new(Vec::with_capacity(16))),
        }
    }
    pub fn update_title(&self, view: Option<usize>) {
        let views = self.views.lock().unwrap();
        if let Some(view) = views.get(view.unwrap_or(self.current_view())) {
            let title = view.update_title();
            self.window.set_title(&format!("{} - {}", title, TITLE));
        }
    }
    pub fn open(&self, source: Source) {
        let view = View::open(source, &self.tags);
        view.setup(self);
        {
            let mut views = self.views.lock().unwrap();
            views.push(view);
        };
    }
    pub fn setup(&self) {
        self.tabs.remove_page(None);
        let filter = FileFilter::new();
        filter.add_pattern("*.md");
        filter.add_pattern("*.txt");
        filter.add_pattern("*.markdown");
        filter.add_mime_type("text/markdown");
        filter.set_name("Markdown");
        let me = self.clone();
        self.tabs.drag_dest_add_uri_targets();
        self.tabs.drag_source_add_uri_targets();
        self.tabs
            .connect_drag_data_received(move |_, _, _, _, data, _, _| if let Some(uri) =
                data.get_uris().into_iter().next() {
                                            let view = View::open(Source::Url(Url::parse(&uri)
                                                                                  .unwrap()),
                                                                  &me.tags);
                                            view.setup(&me);
                                            {
                                                let mut views = me.views.lock().unwrap();
                                                views.push(view);
                                            }
                                        });
        let me = self.clone();
        self.close
            .connect_clicked(move |_| {
                let mut views = me.views.lock().unwrap();
                let index = me.current_view();
                if let Some(view) = views.get(index) {
                    me.tabs.remove(&view.window);
                }
                if views.len() > 0 {
                    views.remove(index);
                }
            });
        let me = self.clone();
        self.new
            .connect_clicked(move |_| {
                                 let view = View::new(Source::Unknown, &me.tags);
                                 view.setup(&me);
                                 {
                                     let mut views = me.views.lock().unwrap();
                                     views.push(view);
                                 }
                             });
        let window = self.window.clone();
        let filter2 = filter.clone();
        let save = self.save.clone();
        let views = self.views.clone();
        let tabs = self.tabs.clone();
        let me = self.clone();
        save.connect_clicked(move |_| {
            let source = {
                let views = views.lock().unwrap();
                views
                    .get(tabs.get_property_page() as usize)
                    .map(|v| v.source.lock().unwrap().clone())
            };
            if source.is_some() {
                let views = views.lock().unwrap();
                views[tabs.get_property_page() as usize]
                    .save(Source::Unknown)
                    .unwrap();
            } else {
                let dialog = FileChooserDialog::new(Some("Select a file"),
                                                    Some(&window),
                                                    FileChooserAction::Save);
                dialog.add_button("Save", 0);
                dialog.add_button("Cancel", 1);
                dialog.add_filter(&filter2);
                let me2 = me.clone();
                dialog.connect_file_activated(move |dialog| {
                    let me = me2.clone();
                    if let Some(filename) = dialog.get_filename() {
                        let views = me.views.lock().unwrap();
                        if let Some(view) = views.get(me.current_view()) {
                            let mut source = view.source.lock().unwrap();
                            *source = Source::File(filename);
                        }
                    }
                    dialog.destroy();
                });
                let me2 = me.clone();
                dialog.connect_response(move |dialog, id| {
                    let me = me2.clone();
                    if id == 0 {
                        if let Some(filename) = dialog.get_filename() {
                            let views = me.views.lock().unwrap();
                            if let Some(view) = views.get(me.current_view()) {
                                let mut source = view.source.lock().unwrap();
                                *source = Source::File(filename);
                            }
                        }
                        dialog.destroy();
                    }
                });
                dialog.show_all();
                dialog.run();
            }
        });
        let me = self.clone();
        self.open
            .connect_button_press_event(move |open, ev| {
                if ev.get_button() == 3 {
                    // Load menu
                    let glade_src = include_str!("../load-menu.glade");
                    // Build from glade
                    let builder = Builder::new_from_string(glade_src);
                    let menu: Menu = builder.get_object("menu").unwrap();
                    let load_url: MenuItem = builder.get_object("load-url").unwrap();
                    let me = me.clone();
                    load_url.connect_activate(move |_| {
                        let glade_src = include_str!("../url-dialog.glade");
                        // Build from glade
                        let builder = Builder::new_from_string(glade_src);
                        let url: Entry = builder.get_object("url").unwrap();
                        let dialog: Dialog = builder.get_object("dialog").unwrap();
                        let me = me.clone();
                        let ok: Button = builder.get_object("ok").unwrap();
                        let dialog2 = dialog.clone();
                        ok.connect_clicked(move |_| {
                                               me.open(Source::Url(Url::parse(&url.get_text()
                                                                                   .unwrap()
                                                                                   .trim())
                                                                           .unwrap()));
                                               dialog2.destroy();
                                           });
                        dialog.show_all();
                        dialog.run();
                    });
                    // Pop it up
                    menu.popup(None::<&Widget>,
                               Some(open),
                               |_, _, _| true,
                               0,
                               ev.get_time());
                }
                Inhibit(false)
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
                let me2 = me.clone();
                dialog.connect_response(move |dialog, id| {
                    let ref me = me2;
                    if id == 0 {
                        if let Some(filename) = dialog.get_filename() {
                            me.open(Source::File(filename));
                        }
                    }
                    dialog.destroy();
                });
                let me2 = me.clone();
                dialog.connect_file_activated(move |dialog| {
                                                  let ref me = me2;
                                                  if let Some(filename) = dialog.get_filename() {
                                                      me.open(Source::File(filename));
                                                  }
                                                  dialog.destroy();
                                              });
                dialog.show_all();
                dialog.run();
            });
        let me = self.clone();
        self.bold
            .connect_clicked(move |_| {
                                 let views = me.views.lock().unwrap();
                                 if let Some(view) = views.get(me.current_view()) {
                                     view.apply_plain_tag(&me.tags.lookup("bold").unwrap());
                                 }
                             });
        let me = self.clone();
        self.h1
            .connect_clicked(move |_| {
                                 let views = me.views.lock().unwrap();
                                 if let Some(view) = views.get(me.current_view()) {
                                     view.apply_line_tag(&me.tags.lookup("h1").unwrap());
                                 }
                             });
        let me = self.clone();
        self.h2
            .connect_clicked(move |_| {
                                 let views = me.views.lock().unwrap();
                                 if let Some(view) = views.get(me.current_view()) {
                                     view.apply_line_tag(&me.tags.lookup("h2").unwrap());
                                 }
                             });
        let me = self.clone();
        self.tabs
            .connect_switch_page(move |_, _, id| { me.update_title(Some(id as usize)); });

        self.window
            .connect_delete_event(|_, _| {
                                      main_quit();
                                      Inhibit(false)
                                  });
    }
}
