use indicatif::{ProgressBar, ProgressStyle};

pub fn make_bar(len: u64) -> ProgressBar {
    println!("");
    let pb = ProgressBar::new(len);

    pb.enable_steady_tick(std::time::Duration::from_millis(80));

    let pb_style = ProgressStyle::with_template(
        "{spinner:.magenta.bold} {pos:>2}/{len:.cyan.bold} {msg:.green}",
    )
    .unwrap()
    // Chain tick_strings to the style, not the bar
    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]);

    pb.set_style(pb_style);
    pb
}
