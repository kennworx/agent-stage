//! Draw one diagram to PNG, so a change can be looked at rather than described.
//!
//! ```sh
//! cargo run -p ags-mermaid --features raster --example shot -- in.mmd out.png
//! ```

#[cfg(feature = "raster")]
fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(src), Some(out)) = (args.next(), args.next()) else {
        eprintln!("usage: shot <in.mmd> <out.png>");
        return;
    };
    let text = std::fs::read_to_string(&src).expect("source");
    let png = ags_mermaid::png(&text, &ags_mermaid::Options::default(), 2.0).expect("draws");
    std::fs::write(&out, png).expect("written");
    println!("{out}");
}

#[cfg(not(feature = "raster"))]
fn main() {
    eprintln!("build with --features raster");
}
