// Release builds run as a GUI app, with no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> iced::Result {
    lexgraph::app::ui::run()
}
