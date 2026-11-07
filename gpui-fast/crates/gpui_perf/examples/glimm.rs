extern crate gpui_fast as gpui;
extern crate gpui_platform_fast as gpui_platform;

#[path = "../src/fast/glimm_demo.rs"]
mod demo;

fn main() {
    demo::run();
}
