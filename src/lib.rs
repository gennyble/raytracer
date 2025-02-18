pub mod material;
pub mod ray;
mod vec3;
pub mod worlds;

use material::{Colour, RGBColour};
use ray::{Camera, HittableList};

pub use vec3::{Point, Vec3};

use rand::Rng;

#[derive(Clone)]
pub struct Renderer {
	viewport: Viewport,
	camera: Camera,
	world: HittableList,
}

impl Renderer {
	pub fn new(viewport: Viewport, camera: Camera, world: HittableList) -> Self {
		Self {
			viewport,
			camera,
			world,
		}
	}

	pub fn pixel(&self, x: usize, y: usize) -> RGBColour {
		//println!("{x} + {y}");
		let mut rng = rand::thread_rng();

		let mut pixel_colour = Colour::default();
		for _ in 0..self.viewport.samples {
			let u = (x as f64 + rng.gen::<f64>()) / (self.viewport.width - 1) as f64;
			let v = (self.viewport.height as f64 - 1.0 - y as f64 + rng.gen::<f64>())
				/ (self.viewport.height - 1) as f64;
			let ray = self.camera.get_ray(u, v);

			pixel_colour = pixel_colour + ray.colour(&self.world, self.viewport.depth);
		}

		RGBColour::from(pixel_colour / self.viewport.samples as f64)
	}

	pub fn line(&self, n: usize) -> Vec<u8> {
		let mut component_vec = vec![0; self.viewport.width * 3];
		for x in 0..self.viewport.width {
			let color = &self.pixel(x, n);

			component_vec[x * 3] = color.r;
			component_vec[x * 3 + 1] = color.g;
			component_vec[x * 3 + 2] = color.b;
		}

		component_vec
	}

	pub fn frame(&self) -> Vec<u8> {
		let mut component_vec = vec![0; self.viewport.area() * 3];
		for index in 0..self.viewport.area() {
			let color = &self.pixel(
				index % self.viewport.width,
				self.viewport.height - 1 - (index / self.viewport.width),
			);

			component_vec[index * 3] = color.r;
			component_vec[index * 3 + 1] = color.g;
			component_vec[index * 3 + 2] = color.b;
		}

		component_vec
	}
}

#[derive(Clone, Copy)]
pub struct Viewport {
	pub width: usize,
	pub height: usize,
	pub samples: usize,
	pub depth: usize,
}

impl Viewport {
	pub fn new(width: usize, height: usize, samples: usize, depth: usize) -> Self {
		Self {
			width,
			height,
			samples,
			depth,
		}
	}

	pub fn aspect_ratio(&self) -> f64 {
		self.width as f64 / self.height as f64
	}

	pub fn area(&self) -> usize {
		self.width * self.height
	}
}
