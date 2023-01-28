use std::{
    fs::File,
    io::{stdout, Write},
    path::Path,
};
use std::{io::BufWriter, sync::Arc};
use std::{thread, time::Instant};

use png::Encoder;

use raytracer::{worlds::*, Renderer, Viewport};

//Image parameters
const ASPECT_RATIO: f64 = 16.0 / 9.0;
const IMAGE_WIDTH: usize = 1920;
const IMAGE_HEIGHT: usize = (IMAGE_WIDTH as f64 / ASPECT_RATIO) as usize;
const SAMPLES_PER_PIXEL: usize = 10;
const MAX_DEPTH: usize = 10;
const NUM_THREADS: usize = 12;

fn main() {
    //Worldgen!
    let (world, camera) = complex_random_scene(ASPECT_RATIO);

    let viewport = Viewport::new(IMAGE_WIDTH, IMAGE_HEIGHT, SAMPLES_PER_PIXEL, MAX_DEPTH);

    let renderer = Renderer::new(viewport, camera, world);

    let before = Instant::now();
    println!("Please hold. Your render is very important to us...");
    write_buffer_as_png("out_lines.png", &render_threaded_lines(renderer));
    println!(
        "Rendering(concurrently) and writing lines as png took {}ms",
        Instant::now().duration_since(before).as_millis()
    );
}

fn write_buffer_as_png<P: AsRef<Path>>(fname: P, buffer: &[u8]) {
    let mut png_encoder = Encoder::new(
        BufWriter::new(File::create(fname).unwrap()),
        IMAGE_WIDTH as u32,
        IMAGE_HEIGHT as u32,
    );
    png_encoder.set_color(png::ColorType::RGB);

    png_encoder
        .write_header()
        .expect("Failed to write png head!")
        .write_image_data(buffer)
        .expect("Failed to write PNG data");
}

fn render_threaded_lines(renderer: Renderer) -> Vec<u8> {
    let arc_renderer = Arc::new(renderer);
    let mut threads = vec![];

    for thread_num in 0..NUM_THREADS {
        let cloned = arc_renderer.clone();
        threads.push(thread::spawn(move || render_lines(cloned, thread_num)));
    }

    let mut component_vec = vec![0; IMAGE_WIDTH * IMAGE_HEIGHT * 3];

    //wait for all threads to finish execution, then fill the component vector
    for handle in threads {
        for (colours, row) in handle.join().unwrap() {
            for (row_index, component) in colours.into_iter().enumerate() {
                component_vec[(IMAGE_WIDTH * (IMAGE_HEIGHT - 1 - row)) * 3 + row_index] = component;
            }
        }
    }

    println!("\rFinished rendering!                         ");
    //todo: why does this still print the last number?
    //gen: because the number starts at col 20 in the `print!` and "Finished rendering!"
    //only takes 19 characters, leaving the space and number in place :D

    component_vec
}

fn render_lines(renderer: Arc<Renderer>, thread_num: usize) -> Vec<(Vec<u8>, usize)> {
    let mut lines = Vec::with_capacity(IMAGE_HEIGHT / NUM_THREADS);
    for j in (0..IMAGE_HEIGHT).rev() {
        if j % NUM_THREADS != thread_num {
            continue;
        }
        print!("\rNow rendering line: {} ", j);
        stdout().flush().unwrap();
        lines.push((renderer.line(j), j));
    }
    lines
}
