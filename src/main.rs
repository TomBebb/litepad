extern crate gtk;
extern crate uuid;
extern crate pulldown_cmark;

mod app;
mod view;

use gtk::*;

use app::App;

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
    // Start running main loop
    gtk::main();
}
