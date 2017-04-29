use view::View;

use gtk::*;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};


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
    pub fn update_title(&self, view: Option<usize>) {
        let views = self.views.try_lock().unwrap();
        if let Some(view) = views.get(view.unwrap_or(self.current_view())) {
            let title = view.update_title();
            self.window.set_title(&format!("{} - {}", title, TITLE));
        }
    }
    pub fn open(&self, path: PathBuf) {
        let view = View::open(path, &self.tags);
        view.setup(self);
        {
            let mut views = self.views.try_lock().unwrap();
            views.push(view);
        };
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
        self.new
            .connect_clicked(move |_| {
                                 let view = View::new(None, &me.tags);
                                 view.setup(&me);
                                 {
                                     let mut views = me.views.try_lock().unwrap();
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
                    dialog.destroy();
                });
                dialog.show_all();
                dialog.run();
            }
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
