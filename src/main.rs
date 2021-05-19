mod vec3;
use rand::{self, Rng};
use vec3::{Camera, HittableList, RGBColour, Sphere, Vec3};

use std::io::BufWriter;
use std::{fs::File, io::Write, path::Path}; //to flush the print! call after each scanline updates

type Colour = Vec3;
type Point = Vec3;

fn main() {
    //Image
    const ASPECT_RATIO: f64 = 16.0 / 9.0;
    const IMAGE_WIDTH: usize = 1920 / 4;
    const IMAGE_HEIGHT: usize = (IMAGE_WIDTH as f64 / ASPECT_RATIO) as usize;
    const SAMPLES_PER_PIXEL: usize = 100;
    const MAX_DEPTH: usize = 50;

    let mut output = [[RGBColour::default(); IMAGE_WIDTH]; IMAGE_HEIGHT];

    //World
    let mut world = HittableList::default();
    world.add(Box::new(Sphere::new(Point::new(0.5, 0.0, -1.0), 0.5)));
    world.add(Box::new(Sphere::new(Point::new(0.0, -100.5, -1.0), 100.0)));

    let camera = Camera::new();

    let mut out_file = BufWriter::new(File::create(Path::new("out.ppm")).unwrap());

    //Render
    write!(out_file, "P3\n{} {}\n255\n", IMAGE_WIDTH, IMAGE_HEIGHT)
        .expect("could not write to file!");
    let mut rng = rand::thread_rng();

    for j in (0..IMAGE_HEIGHT).rev() {
        print!("\rScanlines remaining: {} ", j);
        std::io::stdout().flush().unwrap();

        for i in 0..IMAGE_WIDTH {
            let mut pixel_colour = Colour::default();
            for _ in 0..SAMPLES_PER_PIXEL {
                let u = (i as f64 + rng.gen::<f64>()) / (IMAGE_WIDTH - 1) as f64;
                let v = (j as f64 + rng.gen::<f64>()) / (IMAGE_HEIGHT - 1) as f64;
                let r = camera.get_ray(u, v);
                pixel_colour = pixel_colour + r.colour(&world, MAX_DEPTH);
            };
            output[IMAGE_HEIGHT - j - 1][i] = RGBColour::from(pixel_colour / SAMPLES_PER_PIXEL as f64);
        }
    }
    println!();

    for scanline in &output{
        for pixel in scanline{
            writeln!(out_file, "{}", pixel).unwrap();
        }
    }

    out_file.flush().unwrap();
}
